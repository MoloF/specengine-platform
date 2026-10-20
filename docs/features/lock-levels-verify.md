---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-code]
ref: split from lock-levels 2026-10-20 (Q22); refreshed 2026-10-20 against 7ecc9f6 (S1-S14 fixed in place; provenance a2c62eb); Phase 3 slice 5b-1b (Q5, Q6 2026-10-12; Q13, Q20 2026-10-16; Q25, Q26, Q42-Q47 2026-10-20; decided under the owner's pre-approval, review open); 05 s5.3; 08 AC-2, AC-14
adrs: [ADR-0050]
---

# lock-levels-verify (5b-1b): verify by levels

## Why

5b-1a (`lock-levels.md`, shipped d45dfce) records each entry's `levels` and every level's digest, but verify still judges as v2 did: an item compares `sig` and `body` whatever its levels, so a `[sig]` marker reads a body edit as drift; a `module` or `ron` row compares its file's blob, so any edit of a RON file or a module's file is drift for everything bound in it (05:135: the levels exist "so a refactor is not an alarm the owner learns to ignore"). This slice makes the declared levels count. It changes no lock byte: every digest it compares is already in format 2.

Delivered: verify judging every kind by levels, C = (L_lock ∪ L_mark) ∖ `path`; `code_changed <levels>` and `levels_changed <from>-><to>`; `module` and `ron` judged by digest; JSON `levels`; stored drift rows as verify writes them; a row's `code` without a blob OID; 4a's CRLF limit lifted; 5b-1a's and 5b-2's review carry-overs (RD-9, Q47). Order (08 s2): after 5b-1a and 5b-2 (`lock-rebase.md`, shipped 7ecc9f6), before 5c.

## Description and interactions

- **An agent** sees `code_ahead code_changed <levels>` and `code_ahead levels_changed <from>-><to>` in `spec verify` and in `report_drift`'s evidence, and a `levels` key on every JSON row; `[sig]`, `[path]`, `[deps]` are honoured. No read's `sync` (7a, `bundle-bindings.md`) or Health row (9b) shows them yet.
- **The owner** gets less noise from `module` and `ron` bindings: comments, rustfmt, CRLF and a sibling RON value's edit read `ok`; any code edit in the file of a default-levels module still drifts it (its `body` is the file: `code-hash.md` "File module"). Approving a `levels_changed` item runs accept, which records the new union. New exit 2 in a project hook: a bound module whose file has a parse error, a bound RON value of a file `RonAnalysis` rejects, once its blob changed (accept's refusal, `lock-levels.md` Q31).
- **Crates.** Core `verify.rs`: a pure level comparison (level names and digest strings in, step 4's outcome out); `Comparison` (:769-784) carries that outcome, the judge maps it to a category and detail (:1099), so "the judge never compares hashes itself" stays true; `MarkerFact` keeps `bad_levels`. Core `drift.rs`: docs only (`HashedRow.code`, :339-344). CLI `verify_rows.rs`: `CodeRef::Bound` (:243) becomes one variant per kind (an item's index; an inline `mod`'s `mod_item` index; the file module; a RON value by its comment's byte range, which the walk already holds, :315, :328), the cascade (:698-808) rewritten per "Level-aware verify"; its docs (:30-39, :149-154, :698-701). CLI `verify.rs`: `VerifyRow.levels` (:108-135). CLI `drift.rs`: the stored row (Q44) and `HashedRow.code` (Q45). Digests come from `lock.rs`' `pub(crate)` `level_code` (:1389), `lock_digest` (:1372), `lock_level` (:1377), as 5b-2's rebase does (`lock_rebase.rs:823`, :917): verify's digests are accept's by construction. RD-9 adds code `naming.rs`, `markers.rs`; core `lock.rs`; CLI `code_index.rs`, `symbols.rs`, `lib.rs`, `bump.rs`, `revisions.rs`, `lock.rs`, `lock_rebase.rs`.
- **Unchanged**: every lock byte, the readers and their refusals, accept and rebase; CLI `lock.rs` but RD-9's lines (5b-1a pairs a RON value by `Found.comment`, :757-759, :1352: `Target::Ron` needs no range); the queue's schema; store, MCP (`DriftShape.rows` untyped, `mirror.rs:503`), HTTP (`daemon-keys.json:47` pins only `DriftEvidence`'s keys), UI, plugin and the gate; no new crate or dependency. Nothing is blocked (ADR-0012): exit 2 stops only a project's own hook; `spec check`, the gate and CI never run verify (`spec-verify.md`'s opening).
- **Overridden at shipping**: SW-3's sections; shipped canon is not edited while this is built.

## Data

### Level-aware verify

Q5, Q6, Q42, Q43, Q46. Per row with a format-2 entry E and bound markers: L_lock = E's `levels`; L_mark = the union of the markers' effective levels (no list: `sig`, `body`); C = (L_lock ∪ L_mark) ∖ {`path`}. E holds every level's digest, listed or not (`spec-lock.md` "Entry"), so widening a marker compares recorded digests, and the union stops a narrowed marker hiding an edit until accept records the narrower set. Canonical order: `path`, `sig`, `body`, `deps`. The first that applies, on every side (disk, `--staged`'s index, a commit):

1. A marker's list `Invalid`: the row is not compared (Q43), so 4a's order gives `bad_marker bad_levels` (a `cannot_verify` decided by the walk or the node still comes first); `levels` and `code` null.
2. The blob equals `file_blob`: nothing parsed, `compared` `blob`. Markers bind in their own file, so L_mark moves here only through an ID's resolution (a new alias): L_lock ≠ L_mark reads `levels_changed`; else code unchanged.
3. C = ∅ (both sides `[path]`, so L_lock = L_mark): code unchanged, nothing parsed, `compared` null (Q42). Else this run's digests by accept's functions, each changed file parsed once: an item by `item_levels` at its index; an inline `mod` by `item_levels` at its `mod_item`; the file module by `module_levels`; a RON value by `ron::value_hashes`, paired by its comment's range as accept pairs it; rendered by `level_code` (a RON value: `sig` = `body` = its hash, `deps` the empty set's, Q46). Refused: `cannot_verify item_error`, detail the first `ErrorCategory` of the item or of a module's whole file, or `RonValues::CannotVerify`'s (`code-hash-ron.md` "Entry point"); no tree or no RON pair: no detail (today's `NotHashed`, core :1085).
4. C's digests against E's; those differing, in canonical order, are `changed`; `compared` `ast`. Non-empty: code changed, `code_changed`, detail `changed` `,`-joined (`body,deps`; a RON value `sig,body`, so a `[deps]` RON binding never drifts). Empty and L_lock ≠ L_mark: code changed, `levels_changed`, detail `<L_lock>-><L_mark>`, full sets, `path` included (`sig,body->sig`). Else code unchanged.

4a's matrix stands, "code changed" now steps 2 and 4: spec unchanged `code_ahead code_changed|levels_changed` (Check, accept's hint; Q6); spec changed and raised `code_ahead raised`; changed, not raised `conflict` (Fix). Pre-existing compares the category only: a new detail is not "introduced". `path` is never compared here (5c compares it for a `moved` anchor: `anchor-states.md` "The merged row"). Spec side unchanged:

| Case | Row | `compared` | `levels.changed` | `code` |
|---|---|---|---|---|
| 2, L equal | `ok` | `blob` | null | null |
| 2, L differ | `code_ahead levels_changed sig,body->sig` | `blob` | null | null |
| 3, C = ∅ | `ok` | null | null | null |
| 3, refused | `cannot_verify item_error missing` | null | null | null |
| 4, changed | `code_ahead code_changed body` | `ast` | `["body"]` | `body:b3:…` |
| 4, L differ | `code_ahead levels_changed sig,body->sig` | `ast` | `[]` | null |
| 4, equal | `ok` | `ast` | `[]` | null |

**JSON**: each row gains `levels` after `compared` (16 keys): `{"lock":[…],"markers":[…],"changed":[…]|null}`, null without E or a bound marker, or with an `Invalid` list; `changed` null when no digest was compared. `cascade.by_ast` counts `compared` `ast` of every kind. **Text**: only the detail is new (`<class> | <state>[ <category>][ <detail>] | …`):

```
check | code_ahead levels_changed sig,body->sig | R-12 | implements | crates/game/src/stamina.rs | item crates/game::stamina::regen_system function_item
```

```json
{"node":"R-12","relation":"implements","file":"crates/game/src/stamina.rs","kind":"item","symbol":"crates/game::stamina::regen_system","item_kind":"function_item","state":"code_ahead","category":"code_changed","detail":"body","class":"check","pre_existing":false,"markers":[{"line":14,"rev":2}],"node_rev":2,"lock":{"rev":2,"commit":"2ba8a3b…","verified_at":"2026-10-20"},"compared":"ast","levels":{"lock":["sig","body"],"markers":["sig"],"changed":["body"]}}
```

### Drift evidence

Q13, Q20, Q44, Q45. `drift_hash`'s 18 places (core `drift.rs:347-397`) and `lock_code` stay 5b-1a's (`proposal-drift-item.md` "The item"); place 9 `detail` now carries step 4's detail.

- `code` (`HashedRow.code`, place 17): on any row step 4 found changed, whatever its state (`code_changed`, `raised`, `conflict`; Q45), `<level>:<digest>` per changed level, this run's digest, `,`-joined in canonical order (`body:b3:07c3…,deps:b3:d1e0…`); null for `levels_changed`, steps 1-3 and every row not compared. Never a blob OID: a sibling's or neighbour's edit is the same drift.
- **Stored rows** (Q44): `spec propose drift` stores each row as `spec verify --json` writes it, `levels` included: `VerifyRow`'s own serialization, not `drift_row()`'s copy (CLI `drift.rs:609-638`). Core `DriftRow` (:32-58) stays the read view, no new field: it reads past `levels` and 5c's `anchor`, `StoredJson` keeps them, the store's corruption check never requires them (`proposal-drift-item.md` "The item", **Store**).
- An open item raised before this build reads `evidence changed since it was raised` once; a rejected one is raised again once.

### The pin

Q26. `verify_rows.rs` joins `nothing_outside_the_binder_reads_an_items_state` (CLI `code_refs_walk.rs:1370-1441`) with entries `item_levels(`, `module_levels(` (as `anchor-states.md` "Rules and edge cases" expects); `hash_item(`, `hash_node(`, `hash_bound_items(` banned. For this file only, `HashState` (:48, :684-691) and `item.state` (:640, `unbound()`'s `first_category`) are allowed: the ban (:1395) and the `.state` allow list (:1436) gain a per-file exception. The doc (:1360-1368) updated. Its `#[cfg(test)] mod` lies outside `scanned()` (:952).

### Carry-overs

RD-9 (Q47): from 6b-1's, 5b-1a's and 5b-2's reviews and this refresh (S14); owner pre-approved 2026-10-16 and 2026-10-20. No behaviour change but (a) and (k)'s texts.

| # | File | Change |
|---|---|---|
| a | CLI `bump.rs` `REQUIRED_HINT` (:30) | RD-12 (Q17): when no `required` row has a `write` (each shows `-`): `hint: required: spec.lock vouches for the old text: send the change through the queue (spec propose update) and have the owner approve it with --editorial`; else today's text |
| b | code `naming.rs` `package_label` (:207), CLI `code_index.rs` `dir_text` (:168) | one public function in `specengine-code` (`lib.rs` export) the CLI calls; `code_index.rs` defines no `dir_text` |
| c | CLI `symbols.rs:1` | cites `` `docs/canon/symbol-reads.md` "Lookup" `` (data form, `doc_pointers.rs`), not task spec `symbol-reads` |
| d | CLI `lib.rs:137-138` | the `Exit` bullet re-wrapped (no short "`refs`, `symbols`)," line) |
| e | core `lock.rs` `render_block` (:1030) | `debug_assert!`s: the entry's kind matches the lock's format; `levels` non-empty |
| f | CLI `revisions.rs:332-335` | the `Ok(_)` arm, unreachable (`parse_lock(…, Some(build))` refuses format 1), dropped or commented at that invariant |
| g | CLI `verify_rows.rs` `lock_digest` (:811) | dropped for `lock.rs`' |
| h | CLI `verify_rows.rs` `head_lock` (:1059-1087) | the test of `judged`'s format and recipes (:1077) dropped: the judged lock is always read with `Some(build)` (`verify.rs:241`, `verify_rows.rs:1330`); its clause in the doc (:1066-1067), the `LockFormat` import (:52) with it; a blank `///` between the doc's two paragraphs |
| i | CLI `lock.rs:1437` | `found.levels` moved, not cloned |
| j | CLI `lock.rs:1105` | cites canon `spec-verify-tests`, "Runnable and verifiable", not task spec `verify-tests`, "Verifiable" |
| k | CLI `lock_rebase.rs` `rederive_rust` (:596, `catch_unwind` :603) | narrowed to `parse` and `bind_tree` as accept's sites, or its text ``re-deriving `<file>` at blob <oid> failed`` |
| l | core `lock.rs:1448` `DropReason` | loses the unused `PartialOrd, Ord, Hash` |
| m | CLI `lock_rebase.rs:164` | `RebaseOutcome::written` dropped (set at :395, never read) |
| n | code `markers.rs:138` | doc "(Phase 3: `cannot_verify`)" reads `bad_levels` |

## Rules and edge cases

- WHEN a marker narrows its levels, verify SHALL compare the union with the lock's (Q5).
- WHEN both sides are `[path]`-only, no edit SHALL change the row and its file SHALL NOT be parsed (Q42).
- WHEN a marker's level list is `Invalid`, its row SHALL NOT be compared and SHALL read `bad_marker bad_levels` unless 4a's order decides it earlier (Q43).
- WHEN only a sibling RON value or a neighbour item changes, an item's, an inline `mod`'s or a RON value's row SHALL read `ok`; a file module with `body` in C SHALL read `code_changed body`.
- WHEN only a fingerprint or `body_bytes` differs, no verify row or `drift_hash` SHALL change (`lock-levels.md` Q12).
- WHEN run from two roots, another cwd or over the lock's entries reversed, verify SHALL give byte-identical stdout, stderr and JSON; under `core.autocrlf=true`, LF's states, categories and details, `compared` `ast` (the disk's CRLF blob differs from `file_blob`).
- Core sees level names and digest strings only, never tree-sitter (`build_graph.rs` `core_and_store_have_no_tree_sitter`); no stack, kind or prefix in rows or texts (ADR-0008, Q46).

## Acceptance criteria

rust-developer (M6-M8; M1-M5 are 5b-1a's; cap 3):

- [ ] RD-5 -- "Level-aware verify". Core `tests/verify_judge.rs`, pure cells over core's comparison: a narrowed marker with a body edit is `code_changed body`; a widened marker is `levels_changed`; C = ∅ compares nothing; blob equal and L_lock ≠ L_mark is `levels_changed` by `blob`. New CLI `tests/verify_levels.rs`: `narrowed_marker_with_a_body_edit_is_code_changed`, `widened_marker_is_levels_changed`, `path_only_binding_ignores_a_body_edit`, `path_only_module_ignores_a_parse_error_elsewhere`, `sig_marker_ignores_a_body_edit`. **M6** `sig` and `body` compared whatever the levels -> `sig_marker_ignores_a_body_edit`, in the CLI test and its core cell.
- [ ] RD-6 -- `module` and `ron` by digest. `verify_cannot.rs` through `exits_2_everywhere`: `module_file_error_elsewhere_cannot_verify`, `broken_ron_file_binding_cannot_verify` (detail its first category). `verify_cascade.rs` `ron_sibling_edit_is_ok` replaces `blob_only_binding_change_is_code_ahead` (:141-169): the RON row `ok` by `ast`, the module row `code_ahead code_changed body` by `ast`, its text lines (:167-168) and `cascade` re-derived. **M7** RON judged by blob -> `ron_sibling_edit_is_ok`.
- [ ] RD-7 -- "Drift evidence", CLI `drift_raise.rs`: `assert_item` (:96-106) compares `DriftItem.stored_rows` (store `queue.rs:673`) with verify's JSON rows, `levels` included; `item_code_names_its_changed_levels` (:612) and `regen_drift_hash` (:552-600) carry the detail. Green without edits (Q44): core `lock_model.rs` `lock_code_has_no_fingerprint` (:1173), core `drift_stored.rs`, store `queue_drift.rs`, `queue_follow_up.rs`, MCP `mcp_drift.rs` (`NULL_ROW`), CLI `health_rows`, `drift_approve.rs`. **M8** a blob OID as a module's or RON row's `code` -> `module_neighbour_edit_keeps_the_drift_hash`: a drifted `[sig]` module, then a body edit; a drifted RON value, then a sibling edit; each raised again reads `same evidence`.
- [ ] RD-8 -- "The pin": the edited pin green over `verify_rows.rs`; no mutation run.
- [ ] RD-9 -- "Carry-overs" (a)-(n), in iteration 1, no mutation run: the review finds each as listed; CLI `rev_bump.rs` `required_hint_without_a_line_names_only_editorial` pins (a)'s whole text (either way out named fails it); eval `doc_pointers.rs` green; `code_format_history.txt` gains no line; clippy clean. An item failing review returns to "Later", using no iteration.

test-engineer (temp repos over `verify-ecs` and code-a's golden; no fixture edited: `verify-ecs`' baselines, `by_blob 6`, would all move):

- [ ] TE-5 -- the matrix in `verify_levels.rs`, each cell pinning state, category, detail, `compared`, `levels` and, where not `ok`, `code` through the raised item's `drift_hash` (`regen_drift_hash`'s way). Markers: default, `[sig]`, `[body]`, `[deps]`, `[path]`, two on one key, an inline `mod`, an `Invalid` list. Edits: signature, body, a call added, a comment, rustfmt `max_width = 60`, a neighbour, a marker narrowed, widened. RON: the value vs a sibling; `[deps]`. Module: an inner attribute vs its body.
- [ ] TE-6 -- AC-14 for `module` and `ron` (`verify_cannot.rs`): exit 2 in every mode, plain and `--changed`; the RON error after the marked comment, so its anchor stays `Path`.
- [ ] TE-7 -- `verify_output.rs` `verify_is_byte_identical_across_roots_and_order` (:109) gains a levels scenario (a narrowed marker, a `levels_changed` row) over the lock's entries reversed: byte-identical. CRLF inverted: `verify_cascade.rs` `autocrlf_items_same_module_and_ron_code_ahead` (:354-376) and `verify_states.rs` `lf_lock_over_a_crlf_tree_hashes_items_equal` (:2121-2149) assert LF's states, categories and details, `compared` `ast`.

spec-writer:

- [x] SW-1 -- this spec, split 2026-10-20 (Q22); refreshed 2026-10-20 against 7ecc9f6 (S1-S14 fixed in place; Q42-Q47; RD-9; 08 s2; the "Later" pointers of `lock-levels.md`, `lock-rebase.md`, `spec-lock-rebase.md` re-pointed); `export index`, `check` clean.
- [ ] SW-3 -- at shipping, bytes measured 2026-10-20 (Tier 2 cap 12 288):

| File | Bytes | Changes | Budget |
|---|---|---|---|
| `spec-verify.md` | 11 961 | "States" (code changed by levels, `levels_changed`, Q43), :46 `compared` (C = ∅), "Cascade" (every kind by digest), "Known limits" (:77's CRLF line and :80's "`[sig]` reads …" out, a broken module/RON exit-2 line in) | net ≤ 0, never above 12 100 (5c, 6b-2, 8a-2 add pointers) |
| `spec-verify-output.md` | 9 204 | "JSON" `levels`; "Text" the details | ≤ +400 |
| `proposal-drift-item.md` | 4 339 | "The item": `code` (Q45), **Store** (rows as verify writes them) | ≤ +250 |
| `architecture-code.md` | 11 916 | :58 "As built (5b-1b)" | ≤ +100 |
| `proposal-drift.md` | 8 794 | :68, open items' evidence changes once | ≤ +150 |
| `code-hash-ron.md` | 4 324 | :11 "judged from 5b-1b … until then" to now | ≤ 0 |
| `node-revision.md` | 12 283 | "Raising": "(both ways out)" -> "(its ways out)" (a) | 12 282 |
| `spec-lock-rebase.md` | 4 604 | "Known limits": the `catch_unwind` line out (k) | < 0 |

Also: 05:135 and s5.3's cascade (every kind by digest); 08 s2, AC-2, AC-14; `CLAUDE.md` State; `symbol-reads.md`' "Lookup" heading may go after (c).

## Files

- **rust-developer**: core `src/verify.rs`, `src/drift.rs` (docs); CLI `src/verify_rows.rs`, `src/verify.rs`, `src/drift.rs`. RD-9: code `src/naming.rs`, `src/lib.rs`, `src/markers.rs`; core `src/lock.rs`; CLI `src/{code_index,symbols,lib,bump,revisions,lock,lock_rebase}.rs`.
- **test-engineer**: new CLI `tests/verify_levels.rs`. Edited: `verify_common/mod.rs` (`ROW_KEYS` 15 -> 16, :65-81; `Says`, `row_json`, :388-425, 29 `Says` sites: helpers `verify_states.rs:37`, `symbol_twins.rs:31`, literals `verify_cannot.rs:218`, `verify_states.rs:232`, :2025), `verify_output.rs` (:109, :329, :358), `verify_cascade.rs` (:141-169, :354-376), `verify_states.rs` (:1584-1655 `changed_code_is_code_ahead`, :2121-2149), `verify_modes.rs` (:368, `check` likely 2 -> 1: A3), `symbol_twins.rs`, `verify_cannot.rs`, `drift_raise.rs` (:70-106, :261 `ron_value_changed_raises_its_nodes`, :552-612), `code_refs_walk.rs` (:1360-1441), `rev_bump.rs`; core `tests/verify_judge.rs`. Re-run only: core `drift_stored.rs`, `lock_model.rs`; store `queue_*`; MCP `mcp_drift.rs`; HTTP `drift.rs`; CLI `drift_approve.rs`, `health_rows`, `lock_rebase*.rs`; eval `build_graph.rs`, `doc_pointers.rs`.
- **spec-writer**: SW-3.

## Risks

- **New exit 2 in project hooks**: a bound module whose file has any parse error, a bound value of a RON file with a lexer or structure error; a valid Rust file tree-sitter cannot parse pins its `//!` binding at exit 2. On the pilots 0 of 386 module files; RON 67 of 6 301 anchors on pilot A (1.1 %), none bound; 0 on pilot B. TE-6; a "Known limits" line in place of the CRLF one (SW-3).
- **Evidence churns once**: every open item with a `code_changed` row or a module/RON `code` reads "evidence changed" once (`proposal-drift.md:68`, said at shipping).
- **Hidden test churn**: the tester starts with `verify_common` and the inverted tests.
- **`spec-verify.md` near its cap**: net ≤ 0, ceiling 12 100.
- **5c reads this slice's shapes** (`anchor-states.md`: the steps 1, 3, 4, the `levels` object, the pin's entries): keep them as written here.
- **RD-9 widens review** (about +20 %): the guardrail in RD-9.

## Out of scope

Any lock byte (5b-1a); `spec lock rebase` (5b-2); 5c's anchors and `path` (`anchor-states.md`); `--deep`; reads' `sync` (7a); Health's drift row (9b); an MCP tool, UI or plugin command.

## Later

An RD-9 item failing review returns here, unchanged. The UI mock's `ROW_KEYS` (`ui/src/mocks/drift.smoke.test.tsx:22`) still lists 15 keys: green, stale; a UI nit.

## Open

Decided under the owner's pre-approval, review open:

| Point | Answer | The other answer's cost |
|---|---|---|
| Q5 levels verify compares | the lock's ∪ the markers' | markers only: narrowing hides a body edit |
| Q6 class of `levels_changed` | Check | Fix stops `enforce` CIs on a contract edit |
| Q13 the `code` fact | `<level>:<digest>` per changed level, every kind; null for `levels_changed` and rows equal by blob | the blob OID for module and RON rows: a neighbour's edit is new evidence |
| Q20 stored rows without `levels` | read past, never required | every item raised before reads as corrupt |
| Q25 `DriftRow.levels` | superseded by Q44 | -- |
| Q26 `verify_rows.rs`' pin | entries `item_levels(`, `module_levels(`; `HashState`, `item.state` allowed there only | a code accessor, `unbound()` rewritten |
| Q42 C = ∅ | no parse; `compared`, `levels.changed` null | a `[path]` module exits 2 on a parse error elsewhere in its file |
| Q43 an `Invalid` list | not compared: `bad_marker bad_levels`, `levels`, `code` null, as accept refuses it first | a broken module file reads `cannot_verify` where accept says `bad_levels` |
| Q44 a stored row's `levels` | verify's own serialization stored; `DriftRow` the read view, no field | a `DriftRow` field: `drift_stored.rs:111`'s premise and 4 struct literals in 3 crates change, again for 5c's `anchor` |
| Q45 rows carrying `code` | any row step 4 found changed, whatever its state | `raised`, `conflict` lose the fact; a later edit on them is not new evidence |
| Q46 RON, `sig` = `body` | both named (`sig,body`); a `[deps]` RON binding never drifts | a RON rule inside core's judge |
| Q47 the carry-overs | folded in as RD-9, item by item; a failing item returns to "Later"; no mutation run; scope gains `crates/specengine-code` | one more pipeline round |

Also decided 2026-10-20: M6-M8 exactly (cap 3); no new ADR (ADR-0050 covers it; under ADR-0042 a binding it cannot verify is exit 2: `architecture-code.md:56`); one slice, ≤ 3 iterations; fixtures unchanged. Numbers shared with `lock-levels.md` (Q22-Q24, Q27-Q31) and `lock-rebase.md` (Q32-Q41). Assumptions: A1 HEAD 7ecc9f6 green; A2 the bytes above measured 2026-10-20; A3 `verify_modes.rs:368` goes to 1 since a removed marker comment leaves the module `ok` (the tester confirms).

## Dependencies and ordering

- **Needs** 5b-1a (format 2's levels and digests, accept's hashing, `lock_code`, the golden; canon `spec-lock.md`) and 5b-2 (`head_lock` reads `HEAD`'s lock; canon `spec-lock-rebase.md` "The base"); both shipped 2026-10-20.
- **Before**: 5c (`anchor-states.md`: the steps, `levels` JSON, the pin), 8a-2 (`verify-task.md`: `levels` in rows), 6b-2, 7a (`bundle-bindings.md` **Sync**), 8b, 9b.

## Implementation

Not started. From the split (2026-10-20, `lock-levels.md` Q22): RD-5 <- its RD-4 (M6); RD-6 <- RD-4's RON and module cells (M7); RD-7 <- RD-11 (M8); RD-8 <- RD-10's `verify_rows.rs` half (Q26); RD-9 <- "Carry-overs" and RD-12 (Q47); TE-5 <- TE-2; TE-6 <- TE-3's `module` and `ron` half; TE-7 <- TE-7's verify half; SW-3 <- SW-2's verify half. To record: modules, deviations with reasons.
