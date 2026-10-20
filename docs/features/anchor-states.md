---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5c (WA-1..WA-8; Q1-Q6 2026-10-12; refreshed 2026-10-16 against shipped 4a-4c, 5a, 6a-1, 6a-2 and the ready 5b-1, 5b-2, 6b-2, S1-S21, Q7-Q16; decided under the owner's pre-approval, review open); 05 s5.2 (anchor states, triviality), s5.3 (the sync matrix); 08 s2, AC-15
adrs: []
---

# anchor-states: a renamed or moved binding judged against its old lock entry

## Why

Today (4a, `docs/canon/spec-verify.md` "States") a function renamed or moved together with its marker gives two rows: the old key `broken_binding symbol_gone|file_gone` (Fix) and the new key `unlocked` (Check). So under `enforce-introduced` a pure rename exits 1, on disk and in the advised `--staged` hook alike ("A project's hook"); `spec propose drift` raises a `high` item (`docs/canon/proposal-drift.md` "Raise"); 9b's draft Health row 6 would list both; the agent gets a Fix ("broken by the change", ADR-0042) for a refactor that broke nothing; the owner cannot tell a rename from a deletion plus new code. A manifest change renames qpaths with no code edit (`code-identity.md` "Known limits"), and so does an in-file twin (`symbol-names.md` "Frozen names"): every bound item renamed breaks.

Delivered: a read-only pairing inside verify's judge, on every side it judges (Q7). A lock entry whose key no marker gives any more is paired with a marker binding of the same node and relation that has no entry; equal digests, unique both ways, and the two become one row at the new key, judged against the old entry by 5b-1's levels: a rename only `ok`, a rename with edits `code_ahead code_changed <levels>` or `conflict`, a `[path]` binding `code_changed path`. The anchor answers "where is it", `sync` "does it still match" (05 s5.2, the anchor-state table). No writer, no new `state`, nothing stored, no ADR: ADR-0020 (the marker is the identity, `architecture.md#code-identity`), ADR-0018, ADR-0042 (nothing unverified is `ok`), ADR-0046 (never stored, `architecture-code.md#sync-in-reads`); ADR-0041, ADR-0050 stand; ADR-0012 is helped. Following code with no marker would amend ADR-0020: 12b's ground (08 s2).

Order (08 s2; `code-tools.md` Q28): … 6b-1 -> 5b-1a -> 5b-2 -> 5b-1b -> **5c** -> 6b-2 -> 6c -> 7a; size M, not split ("Dependencies and ordering").

## Description and interactions

- **An agent** running `spec verify --changed` or a `--staged` hook after a rename sees nothing (an `ok` row, `11 ok (1 moved)`), or one Check row with a `moved from` line when it also edited; `report_drift` at a pure-rename commit raises nothing. **The owner** sees the move, re-keys at leisure with `spec lock accept <ID>`, and sees an `ambiguous` Fix row's candidates instead of a bare `symbol_gone`.
- **Re-keying stays with accept** (WA-2): its merge writes the new key `new`, the vanished one `dropped`, a fresh `commit` and `verified_at` (`spec-lock-accept.md` "Steps" 6); 5b-2's rebase keeps keys byte for byte, so it never pairs. 5c has no write door.
- **Crates** (WA-7: core sees keys, digests, `body_bytes` only). Core: a new pure module, the pairing over an opaque group key and digest records (Q13), so 7a and 7b call it as is; its copy of the threshold pinned to the code crate's `TRIVIAL_BODY_BYTES` (`code-hash.md` "Fingerprints": "5c pins a copy"); `drift.rs` (`HashedRow.moved_from`). CLI: `verify_rows.rs`, one seam every judge side calls (Q7), B hashed there (Q15); `verify.rs` (text, hints, JSON, `delta`); `drift.rs`. Code: none (`code-hash.md` "Entry points" exports what 5c reads). Fixtures. **Unchanged**: store, MCP (drift rows pass untyped), HTTP (`fixtures/daemon-keys.json` pins only `DriftEvidence`'s `hash`, `rows`, `more`), UI, plugin, the gate, `spec check`, the index, the queue's schema, `spec.lock` format 2, `specengine.toml`; no new crate or dependency.
- **Overridden at shipping** (shipped canon is not edited while built): the canon sections SW-2 lists, each by the section here that changes it; 5b-1's "Level-aware verify" "`path` is never compared". 5b-1 reserves no `anchor` (Q9).
- **Neighbours** pointing here: 4c-1 shipped the slot (`proposal-drift-item.md` "The item" place 18; "Store": a later build's `anchor` is no corruption); 6a-2 the index's fingerprints (`symbol-index.md` "Tables"); 7a `bundle-bindings.md` (`anchor`, `moved_from` per binding row, through this core pairing, Q13); 7b `impact.md` (`symbols.renamed`, Q14); 6b-2 `verify-node.md` (`anchor`, `--symbol` on a moved row's `from`, its Q34); `lock-levels.md`, `lock-rebase.md` "Out of scope"; 08 s2. **9b, 8b: no edit.** 9b passes rows verbatim, an unknown word raw (`health-drift.md` "`drift`"); if it pins row keys in `daemon-keys.json` first, 5c adds `anchor` there. 8b: a move stales evidence (`tested-evidence.md` "Bound hash"), never followed.

## Data

No file, table, key or schema changes but verify's output and the drift hash's place 18 (4c-1's `null`).

### The pairing

WA-1, WA-4, WA-5, Q2. **Sides** (Q7): wherever verify's judge runs, each side alone, its own entries with its own markers, before its cascade: the disk (plain, `--changed`); `HEAD` (`--changed`'s and `--staged`'s base, with the current lock: `spec-lock-rebase.md` "The base"); the index (`--staged`); one commit (`judge_commit`: `spec propose drift`, MCP `report_drift`). One seam in `verify_rows.rs`; 6b-2's `--node` pairs before its filter.

Per **group** (node, relation, kind, item_kind; to core an opaque key, Q13), T = `TRIVIAL_BODY_BYTES` (124 today, 08 AC-15; TE-1 may move it, Q12):

- **E**: the group's lock entries whose node resolves (not `node_gone`), whose key no marker gives and whose code fact is `file_gone` or `symbol_gone` (`spec-verify.md` "States"), of kind `item` or `module`, with stored `body_bytes ≥ T`. Its `body_hash`, `skeleton_hash`, `quote_hash` are format 2's (`lock-levels.md` "Format 2"; WA-8).
- **B**: the group's marker rows with no entry at their key, lockable now on the code side (5b-1's "Accept" rows `levels` and binding: levels valid, unambiguous, hashed; no `key_problem`; WA-5), with `body_bytes ≥ T`; digests and fingerprints from 5a-1's `item_levels`, `module_levels` over that side's bytes, as accept's. The markers' revisions do not matter: a lagging marker is a candidate (SW refinement; the merged row then reads `spec_ahead marker_behind`).

1. C(e) = every b with an equal `body`; none: every b with b.file = e.file and an equal `skeleton` (05 s5.2: a rename is "symbol vanished + a symbol with the same skeleton appeared in the same file").
2. |C(e)| > 1: keep those with an equal `quote`, only when exactly one is left.
3. R(b) = {e : b ∈ C(e)}, with the same quote rule.
4. **MOVED** iff C(e) = {b} and R(b) = {e}. Else **AMBIGUOUS** if C(e) ≠ ∅ (candidates C(e)), else **LOST**.

- **EXACT**: an entry and a marker binding share the key.
- **Deterministic**: over sets, output in 4a's key order (`spec-verify.md` "Rows"); keys root-relative; no line, path string, time or map order goes in.
- **Cost** (WA-1): candidates are the side's own marker rows: no tree-wide search, no read of 6a's index. A file is parsed for B only if it holds a B of a group with an unpaired E, once per side, shared with the cascade's re-parse (`spec-verify.md` "Cascade").

### Anchors per row

| `anchor.state` | Row | `from` | `candidates` |
|---|---|---|---|
| `exact` | an entry and a marker binding at its key, any state (the null cases below excepted) | null | null |
| `moved` | the merged row ("The merged row") | e's `file`, `kind`, `symbol`, `item_kind` | null |
| `ambiguous` | e's row, 4a's `broken_binding file_gone\|symbol_gone`, Fix (Q3) | null | C(e), the same four fields each, key order |
| `lost` | an entry's `broken_binding` neither moved nor ambiguous: C(e) = ∅, or never followed (RON, `body_bytes < T`, `marker_gone`; WA-1, WA-4) | null | null |

`anchor` null, nothing looked for: a row without an entry (`unlocked`, keyless, a marker-only Fix, an unpaired or contested B), `node_gone` (Q4), the cannot-verify facts `hidden_by_error`, `unreadable`, `item_error`.

### The merged row

Key, markers from b; entry e; `compared` `ast` (b was hashed for the pairing; no blob step). 4a's matrix and 5b-1's "Level-aware verify" steps 1, 3, 4 judge it with b's digests against e's, plus: `path` is changed iff the anchor is `moved` and `path` ∈ L_lock ∪ L_mark (`code-hash.md` "Levels": `path` is the key, never hashed), first in canonical order (`code_changed path,body`). e's own row is gone; `counts.rows` drops by one per pair. Q1: unchanged levels read `ok`.

### Drift evidence

Q6, Q8. `drift_hash` keeps `specengine-drift/v1` and its 18 places (`proposal-drift-item.md` "The item"); `lock_*` are e's. `code` (5b-1's, `lock-levels.md` "Drift evidence") holds digest levels only: a `code_changed path` row's is null, a `path,body` row's `body:<digest>`. The move enters through place 18: `moved_from` (core `HashedRow.moved_from`) = `anchor.from` as `[file, kind, symbol, item_kind]`, else `null`. A stored row without `anchor` reads as stored ("Store").

### Output

WA-6: no new state; the anchor sits beside 5b-1's judgement, so 4c-1's triggers and 9b's label map stand.

- **JSON**: every row gains `anchor` after 5b-1's `levels`, `{"state":…,"from":{…}|null,"candidates":[{…}]|null}` or null. `delta` (over rows not Pre-existing, as 4a): `bindings.moved` `[{"from":<e's six key fields>,"to":<the row's>}]`; `ambiguous` = rows with an `ambiguous` anchor ∪ 4a's `ambiguous_*` rows; `lost` = `broken_binding` rows not `ambiguous`. `moved`, `ambiguous`, `lost` are disjoint; a moved `code_ahead` or `conflict` row is in `stale` too (SW refinement). `counts` unchanged.
- **Text** (Q5, Q16): a non-`ok` moved row gets the line `  moved from <file> | <kind> <symbol>[ <item_kind>]` under it; an ambiguous row one `  candidate <file> | <kind> <symbol>[ <item_kind>]` per candidate; `ok` rows stay unprinted, the summary's `<n> ok` gains ` (<m> moved)`, `ok` rows whose anchor is `moved`, omitted at 0. When any row is moved (`ok` too), once after 4a's per-state hints, before the drift hint: `hint: moved: commit, then spec lock accept <ID> on a terminal locks the new key and drops the old`, "Hints"' form `hint: <state>[ <category>]: <step>` taking one anchor word (Q16).

```
check | code_ahead code_changed body | R-12 | implements | crates/game/src/stamina.rs | item game::stamina::regen_tick function_item
  moved from crates/game/src/stamina.rs | item game::stamina::regen_system function_item
fix | broken_binding symbol_gone | R-20 | implements | crates/game/src/movement.rs | item game::movement::step function_item
  candidate crates/game/src/movement.rs | item game::movement::step_a function_item
  candidate crates/game/src/movement.rs | item game::movement::step_b function_item
hint: moved: commit, then spec lock accept <ID> on a terminal locks the new key and drops the old
verify: blocked [enforce]: 1 fix, 1 check, 0 cannot, 11 ok (1 moved); 9 by blob, 3 by ast; 0 skipped
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

### Fixtures

Q11, Q12. `fixtures/anchor-states/` with `expected.json`: each TE-1 scenario runs in a temp git repo copied from it, locked by `spec lock accept <ID>` (format 2), committed, then changed; no digest hand-written. The threshold bodies are written by the test at T-1 and T `body_bytes`, asserted through `item_levels`; no file, test name or expectation holds the number.

**Churn**: JSON rows gain `anchor` in 10 test files: CLI `verify_states`, `verify_cascade`, `verify_cannot`, `verify_staged`, `verify_common`, `drift_approve`; MCP `mcp_drift`; store `queue_drift`; core `drift_stored`; eval `ast_hash_cli`.

## Rules and edge cases

- WHEN an entry's key is given by no marker, the system SHALL follow it only to a marker binding of its group by a mutual unique match, and SHALL never follow a trivial body (either side), RON, `marker_gone`, `node_gone` or a cannot-verify fact (05 s5.2: "a missed follow heals as STALE, a false follow lies forever").
- WHEN verify judges a side (the disk, `HEAD`, the index, one commit), it SHALL pair on that side alone, before its cascade, so a committed pure rename reads the same under `--changed`, `--staged` and a sweep (Q7).
- WHEN a pair is MOVED, verify SHALL give one row at the binding's key judged against the entry, and SHALL write nothing: no lock byte, index row, queue row or file (WA-3, ADR-0046).
- WHEN only `path` changed on a moved row, its drift `code` SHALL be null, the move carried by `moved_from` alone (Q8).
- WHEN a row cannot be verified, its anchor SHALL be null and it SHALL never be `ok` (AC-14, unchanged).
- WHEN run from two roots, another cwd, `core.autocrlf=true` or over reversed entries, rows, anchors and output SHALL be byte-identical.
- **The pin** (Q15) `nothing_outside_the_binder_reads_an_items_state` (CLI `code_refs_walk.rs`): `verify_rows.rs` hashes B by `item_levels(`, `module_levels(`, never `hash_item(`, `hash_node(`; the allow list (`judged`, `levels`) gains `anchor` (the test-engineer's edit); a new hashing CLI file joins it.
- Genre-neutral: no stack, kind or prefix in the pairing, rows or hints; no Bevy rule (one-line `build` fns fall under T).

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- the pairing, pure (core), T read from the constant (Q12). M: skeleton matched across files -> `skeleton_step_stays_in_the_entry_file`; uniqueness checked one way -> `contested_candidate_moves_nothing`; quote ignored -> `twins_with_distinct_quotes_each_move`; two left after the quote taken -> `twins_with_equal_quotes_stay_ambiguous`; skeleton tried after a body match -> `body_match_wins_over_skeleton`; `<=` for `<` -> `body_under_the_threshold_is_never_followed` (T-1), `body_at_the_threshold_moves` (T); a trivial candidate -> `trivial_candidate_is_never_followed`; another relation or kind -> `candidates_share_the_group_key`; order-dependent -> `pairing_ignores_input_order` (a property test over permutations); a node-typed signature -> `pairing_takes_an_opaque_group_key` (a plain string key; Q13).
- [ ] RD-2 -- never followed. M: RON -> `ron_entry_is_lost`; `marker_gone` -> `lost_marker_is_not_followed`; `node_gone` -> `gone_node_is_not_followed`; `hidden_by_error` -> `hidden_symbol_stays_cannot_verify`.
- [ ] RD-3 -- verify ("The merged row", "Drift evidence"). M: two rows kept -> `moved_binding_is_one_row`; `path` ignored -> `path_marker_reads_a_move_as_code_changed`; `path` changed on an exact row -> `exact_path_binding_stays_ok`; an edit judged without the entry -> `renamed_and_edited_is_code_ahead`; `HEAD` side not paired -> `committed_move_is_pre_existing`; a `path:` token in `code` -> `path_change_adds_no_code_fact` (Q8).
- [ ] RD-4 -- output. M: `moved` left null -> `delta_lists_moved_pairs`; a row in two of `moved`, `ambiguous`, `lost` -> `delta_lists_are_disjoint`; place 18 left null -> `moved_row_hash_carries_moved_from`; `anchor`, a sub-line or the `moved` hint missing or out of place -> verify's JSON and text goldens.
- [ ] RD-5 -- cost, read-only. M: every unlocked binding hashed -> `no_lone_entry_parses_no_extra_file` (a parse counter, as `one_parse_per_rederived_file` in `code_refs_walk.rs`); an anchor persisted -> `verify_writes_nothing`.
- [ ] RD-6 -- pins. M: core's copy of T drifts -> `trivial_threshold_is_the_code_crates`; tree-sitter in core -> `build_graph.rs` `core_and_store_have_no_tree_sitter`; a new dependency -> `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`; `hash_item(` in `verify_rows.rs` -> `nothing_outside_the_binder_reads_an_items_state`, green with B hashed (Q15).

test-engineer:

- [ ] TE-1 -- `fixtures/anchor-states/` ("Fixtures"), one locked-by-accept, committed, then changed scenario each, every row's state, class and anchor pinned:

| Scenario | Expected rows |
|---|---|
| rename within the file | one `ok`, `moved` |
| move to another file | one `ok`, `moved` |
| rename plus a local renamed (skeleton step) | one `code_ahead code_changed body`, `moved` |
| move across files plus an edit | `broken_binding` `lost`; `unlocked` null |
| bodies of T-1 and T `body_bytes`, renamed | T-1: `broken_binding` `lost` + `unlocked`; T: `ok` `moved` |
| twins, distinct quotes, both renamed | two `ok`, `moved` |
| twins, equal quotes, both renamed | two `broken_binding` `ambiguous`, 2 candidates each; two `unlocked` |
| a contested candidate (two entries, one binding) | two `broken_binding` `ambiguous`, 1 candidate; one `unlocked` |
| a marker moved, the old item stays | `broken_binding marker_gone` `lost`; `unlocked` |
| a locked impl gaining an unmarked in-file twin (6a-1: `impl Tally` -> `impl Tally #1`) | one `ok`, `moved`, paired by `body` |
| an ordinal shift: an unmarked twin inserted above two locked twins of one node | never paired: the old `#1` `broken_binding marker_gone` `lost`, the old `#2` compared with the twin now there; `#3` `unlocked` |
| a RON path renamed | `broken_binding marker_gone` `lost`; `unlocked` |
| a `//!` file renamed | `ok`, `moved`, kind `module` |
| a `[path]` binding moved | `code_ahead code_changed path`, `moved` |
| a rename by the manifest only (a lib added) | every bound item of `main.rs` `ok`, `moved` |
| a lagging marker, renamed | `spec_ahead marker_behind`, `moved` |
| an ID renamed to an alias | `broken_binding node_gone`, null; `unlocked`, null |

- [ ] TE-2 -- AC-15 end to end on every side (Q7), `enforce-introduced`: a pure rename uncommitted, `spec verify --changed` exits 0 (4a: 1; M: the pairing off); staged, a `--staged` hook exits 0 (M: off on the index side); committed, `spec propose drift` at it stores no item (M: off in `judge_commit`).
- [ ] TE-3 -- byte-identical JSON and text across two roots, the lock's entries reversed, LF vs `core.autocrlf=true` checkouts.
- [ ] TE-4 -- read-only: after every TE-1 scenario `spec.lock` bytes, `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, refs unchanged.

spec-writer:

- [x] SW-1 -- this spec (2026-10-12, the neighbours pointing here); refreshed 2026-10-16 (S1-S21, Q7-Q16): `impact.md` re-pointed (Q14); the stale `spec-lock.md` and CLI README figures in `lock-levels.md`, `lock-rebase.md`, `verify-node.md`.
- [ ] SW-2 -- at shipping, bytes measured 2026-10-16 (caps 12 288 Tier 2, 10 240 Tier 1), Q10: a new Tier 2 `docs/canon/spec-verify-anchors.md` (the pairing, anchors, the merged row, the evidence; about 3 KB), its line in `docs/index-canon-spec.md` (2 592); `spec-verify.md` (11 710 before 5b-1, 5b-2 and 6b-2 write it: pointers only in "Rows", "States", "Cascade", "Base", ≤ 150 B); `spec-verify-output.md` (5 380: "Text" sub-lines and ` (<m> moved)`, "Hints" the `moved` line, "JSON" `anchor`, `delta.bindings`; about 1.2 KB); `proposal-drift-item.md` (4 095: "The item" `moved_from` filled); `symbol-names.md` (8 464: "Frozen names"' "Never all `ok`", false once a vanished twin key pairs `ok` `moved`; "by `body`" -> "by fingerprint, one to one"); `code-hash.md` (11 645, 643 B left: "Fingerprints"' 5c pointers, net ≤ 0); `architecture-code.md` (10 711: "As built (slice 5c)" under `#verify`, about 120 B); `code-identity.md` (10 471: "Known limits"' manifest-rename line, an optional clause); the core README (9 968, 272 B left: a module pointer, net ≤ 0) and the CLI README (9 086: a pointer); `CLAUDE.md` State (5 742 of 16 384); 05 s5.2 (the anchor-state table's `MOVED` "marker lost" -> "marker found at another key"; the fingerprint paragraph; "Reply to every write"'s "`moved` 5c"), s5.3 (the sync matrix's "symbol renamed"); 08 s2, AC-15 met.
- [x] SW-3 -- no ADR (ADR-0012, ADR-0018, ADR-0020, ADR-0042, ADR-0045, ADR-0046, ADR-0050 hold); `export index`, `check` clean.

## Risks

- **Delayed alarm**: an `ok` move leaves the old key in the lock; an edit after a move across files then cannot match (the skeleton step stays in the entry's file), and `broken_binding` names a long-gone symbol. Through the queue too: an open pre-5c item for a committed rename "reads ok at HEAD now" (`proposal-drift-item.md` "Dedup") while the lock keeps the old key. Mitigations: the `moved` hint, Q1.
- **Reads vs verify**: 7a reads the index's digests and shows `anchor`, `moved_from` (`bundle-bindings.md` "The binding row"); unless it calls this core pairing (Q13), reads and verify give two answers.
- **Weak quote tie-break**: `quote`'s `before` is often the marker comment, shared by twins bound by one node; TE-1's figures decide whether a step stays, each failing safe to AMBIGUOUS or LOST.
- Pilots hold no marker (`code-hash-eval.md` "Known limits"): proven on fixtures only.

## Out of scope

Following a lost marker (`marker_gone`) or code without a marker (12b); ID renames (`node_gone`: accept drops the gone node); RON (never followed, `code-hash-ron.md` "RON value"); storing anchors; the pairing in accept or rebase rows; `symbols.renamed` (7b's, on this pairing: Q14); evidence following a move (8b); `--deep`; per-branch ratios (Phase 4); UI.

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q1 a moved binding, levels unchanged | `ok` (AC-15 "rename is not drift"; `[path]` the opt-in, 05 s5.2's "lives here" note) | a Check, Health row and drift item per rename, `path` meaningless; the lock re-keyed sooner |
| Q2 where candidates come from | marker bindings of the same node and relation | anything in the tree (05 s5.2's `MOVED`, "one definition in the project", literally): whole-tree hashing each run, the twin trap, an ADR-0020 amendment |
| Q3 an AMBIGUOUS row's class | 4a's `broken_binding` (Fix), with candidates | as Check, an `enforce` project passes copies verify cannot vouch for |
| Q4 follow `node_gone` (an ID now an alias) | no | a second identity path inside 5c |
| Q5 moved `ok` rows in text | the summary's count only | every rename printed |
| Q6 the `drift_hash` slot | shipped by 4c-1 (place 18, `null`); 5c fills it ("Drift evidence") | `specengine-drift/v2` later, an "evidence changed" note per open item |
| Q7 sides | every side verify's judge runs, one seam in `verify_rows.rs` ("The pairing"); 6b-2 inherits it | disk and `HEAD` only: a `--staged` hook exits 1 on a committed rename, a sweep raises a `high` item, no item carries `moved_from` |
| Q8 the drift `code` fact for `path` | digest levels only; the move enters through `moved_from` ("Drift evidence") | `path:<old symbol>` in `code`: the move counted twice, a new token form in 4c-1's evidence |
| Q9 5b-1 reserving `anchor` | no: 5c adds it after `levels` | a null key in 5b-1's goldens 5c rewrites to `exact`: 10 suites churn twice |
| Q10 canon home | a new Tier 2 `docs/canon/spec-verify-anchors.md`; `spec-verify.md` pointers only; the output in place in `spec-verify-output.md` (SW-2) | in `spec-verify.md`: 578 B left, which 5b-1, 5b-2, 6b-2 contest: over 12 288 at shipping, an unplanned split |
| Q11 TE-1's locks | made by `spec lock accept` (format 2) in a temp repo, then changed | hand-written digests break `lock-levels.md` "Fixtures and the golden", rot with any recipe change |
| Q12 the threshold in tests | `TRIVIAL_BODY_BYTES`: bodies at T-1 and T, names without the number | 123 and 124 baked in: a TE-1 change to AC-15 breaks the fixture, the test names, the pin |
| Q13 core API | an opaque group key and digest records; no node or relation type in core | a node-shaped API: 7b writes a second follower for `renamed` |
| Q14 7b's `symbols.renamed` | 5c fills `bindings.moved` only; `renamed` stays 7b's, on Q13's function | 5c fills `renamed` over all code: whole-tree hashing every run, against Q2 |
| Q15 the pin | its allow list gains `anchor`; B hashed by `item_levels(`, `module_levels(` in `verify_rows.rs` | the pin goes red, or hashing moves to an unpinned file |
| Q16 the hint | "locks the new key and drops the old", between the per-state and drift hints ("Output") | "moves its entry": the owner expects the old verification to carry over |
| Size, order (owner) | M, not split; after 5b-1b ships (08 s2); no gate beyond 5b-1's | a split leaves no whole product: the sub-line is 6b-2's Q34, AMBIGUOUS falls out of the same check |

Q1-Q6 (2026-10-12) and Q7-Q16, size and order (2026-10-16, the refresh `ref` names) decided under the owner's pre-approval, review open. S-n: that refresh's stale points; WA-n: the 2026-10-12 analysis' working answers, WA-5..WA-7 assumed; "SW refinement": the spec-writer's.

## Dependencies and ordering

- **From 5b-1** (`lock-levels.md`): "Format 2"'s `levels`, digests and fingerprints; `parse_lock(…, Some(build))` ("Readers"); the judge's steps 1, 3, 4 and `levels` JSON (5b-1b: `lock-levels-verify.md` "Level-aware verify"); the lockability rows as a predicate for B ("Accept"); `verify_rows.rs`' `item_levels` re-parse and its pin (5b-1b); `HashedRow.code` ("Drift evidence"); the accept-made golden.
- **From 5b-2** (`lock-rebase.md`): "The base" (`HEAD` judged with the current lock, so its side pairs on format 2); `read_head`; keys kept byte for byte, so a rebase never pairs.
- **From 5a-2's TE-1** (the owner's run; `code-hash-eval.md` "Measured": it gates 5b-1 and "AC-15's 124 before 5c"): the threshold (`trivial_at."<t>"` at 64-256); stability (`stable_pct.<p>.{skeleton, body_bytes}`, `rename.kept` at 100.0, else a 5a-1 recipe change before the first format-2 lock); tuning (`skeleton_groups_in_file`, `quote_separates`, `body_groups`, `rename.followable`); here 45.2 % trivial, 147 body groups. Everything structural is decided now; the figures may only drop the skeleton or quote step.
- **Order**: after 5b-1b ships (08 s2), which rewrites `verify_rows.rs` just before; no gate of its own (5b-1 waits for the whole TE-1: `lock-levels.md` Q9); then 6b-2, 6c, 7a. **Independent in code**: 4c, 8b, 9b.
- **Assumptions** (2026-10-16): 5b-1, 5b-2 ship as refreshed that day; one constant, the code crate's; no real format-1 lock, no pilot marker (`code-hash-eval.md` "Known limits").

## Implementation

Not started. The 2026-10-16 refresh: RD-1's threshold tests parametrised (Q12), plus Q13's; RD-3, RD-4 gain Q8's and place 18's tests; RD-5, RD-6 name their precedent, graph tests and pin; TE-1's RON and twin rows fixed, locks by accept (Q11); TE-2 on every side (Q7); old SW-1, SW-2 -> SW-1, SW-3 -> SW-2. To record: modules, the parse counter's figures, deviations, reasons.
