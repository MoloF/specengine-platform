---
class: spec
status: shipped
scope: [crates/specengine-code, fixtures]
ref: Phase 3 slice 5a-1b, split from hash-levels 2026-10-15 (its Q12; WA-3; Q7, Q15, R2, R2b, R3 decided under the owner's pre-approval); 05 s5.2; 08 s2, AC-14; Rust 1 iteration, review accepted, tests 1 round
shipped: 2026-10-16
adrs: []
---

# hash-levels-ron: the value hash of an anchored RON value

## Why

A format-1 `ron` binding is judged by `file_blob` alone: any byte of its file and an autocrlf checkout stale it (`docs/canon/spec-lock.md` "Lockability"; `spec-verify.md` "Known limits"). 5b judges it by the anchored value's hash instead (`lock-levels.md` "Format 2": `ron` sig = body = `value_hash`). Once a lock records that hash, its bytes change only with a `LEVEL_RECIPE` bump and a `spec lock rebase` of every lock (ADR-0050), so they are fixed and pinned before 5b.

Split from 5a-1 (`hash-levels.md` Q12) so a third iteration on one slice cannot leave the Rust levels unshipped. Order (08 s2): 5a-1 -> 5a-1b -> 5a-2. No ADR.

## Description and interactions

Canon: `docs/canon/code-hash-ron.md` (all of it; `code-hash.md` "Entry points" and the code README "Hash recipe" point there); the code README's `ron` row; `code-identity.md` "RON binding", "Open"; 08 s2. Nobody sees the digests yet: 5a-2 measures them, 5b-1 records and judges them, 5b-2 rebases them. `analyze`, `comment_anchors` and their types unchanged (Q15); all RON knowledge stays in `specengine-code` (ADR-0008); core, store, CLI untouched; no new dependency.

## Data

### Entry point

`docs/canon/code-hash-ron.md` "Entry point": keys, source order, the whole-file refusal (Q7), the span internal to `ron` (Q15).

### RON value

`docs/canon/code-hash-ron.md` "RON value": the stream, the span (L1, L2, L8), the drops (L3, L7), the CR rule (L4), the map-key comment (R2, R2b), the levels (WA-3), the example. Closes `code-identity.md` "Open" (level meaning in RON).

## Rules and edge cases

- WHEN the same bytes are hashed in two runs or in reverse file order, the system SHALL give the same digests: no path, line, offset or time enters a stream.
- One lex and one structure walk per file, never a re-lex per comment. The value pass adds no recursion; the walk is the shipped one, capped at `MAX_DEPTH` 512 within a 2 MB stack (R3).
- WHEN `analyze` or `comment_anchors` runs on any source, the system SHALL give what it gave before this slice: the shipped `ron_*` tests of `specengine-code` pass unedited.
- Code comments cite `crates/specengine-code/README.md` "Hash recipe", never this spec's headings.

## Acceptance criteria

rust-developer:

- [x] RD-1 -- "RON value". M: comments hashed -> `ron_comment_edit_keeps_the_value_hash`; sibling tokens -> `ron_sibling_edit_keeps_the_value_hash`; trailing comma kept -> `ron_trailing_comma_keeps_the_value_hash`; a lexer error hashed -> `broken_ron_file_value_cannot_verify`; a structure error hashed -> `ron_structure_error_refuses_every_value`; CR kept -> `ron_value_hash_equal_under_lf_and_crlf`; lexer drift -> `ron_value_stream_is_pinned`.
- [x] RD-2 -- "Entry point". M: a re-lex per comment -> `ron_value_hashes_linear_in_comments` (best of three, as `ron_cost.rs`); markers only, or a key other than `CommentAnchor::comment` -> `unmarked_path_comments_are_keyed_as_comment_anchors` (SW refinement). The code-reviewer: no public field added to `Anchor`, `RonMarker`, `CommentAnchor` (Q15).

test-engineer:

- [x] TE-1 -- `.ron` inputs in `fixtures/hash-levels/ron/`, `expected.json` extended: per value form (L13) an equal and a different pair, LF and an in-memory CRLF copy, files `RonAnalysis` rejects (lexer, structure) refusing every value, each pair naming its mutation. Eval `hash_levels_ron_fixture.rs` `te1_ron_*` (6 tests).
- [x] TE-2 -- determinism: two runs, reversed file order. `te2_two_runs_and_reversed_file_order_give_the_same_bytes`, `te2_no_offset_line_or_path_enters_a_value_hash`.

spec-writer:

- [x] SW-1 -- this spec (split from `hash-levels.md`, 2026-10-15, its Q12).
- [x] SW-2 -- at shipping: `code-hash.md` had no room (11 551 B, the section ≈ 826 B): Tier 2 `docs/canon/code-hash-ron.md`, `code-hash.md`'s RON line its pointer, `docs/index-canon.md` re-measured; `code-identity.md` "Open" without "level meaning in RON"; the code README's "Hash recipe" and `ron` row name `value_hashes` (net -12 B); 08 s2; `CLAUDE.md` State; `lock-levels.md` the RON `deps` note.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Out of scope

The Rust levels and fingerprints (5a-1); the eval and pilot figures (5a-2); CLI, lock, verify, ADR; a public range field (Q15); a RON fingerprint, following RON or a RON shape level; the RON grammar itself (`code-identity.md` "RON binding").

## Open

Decided under the owner's pre-approval: Q7 (2026-10-13) any `RonAnalysis` rejection refuses every value of the file, the detail the first `error_categories` entry (lexer errors only: a structure error can shift a value's range, a false `ok`); Q15 (2026-10-15) the value ranges internal to `ron`, `value_hashes` the one way out (a field on `RonMarker` or `CommentAnchor` breaks struct literals); R2, R2b, R3 (review 1, 2026-10-16): "Implementation". Q-numbers are shared with `hash-levels.md`. Left to 5b-1: a public source for the RON `deps` digest (5b-1a: `empty_deps_digest()`, `lock-levels.md` "The empty set's `deps`").

## Implementation

| Module | What it does |
|---|---|
| code `ron/mod.rs` | `RonValues`, `value_hashes` (one lex, one walk, digests memoised per value), `value_digest` (the stream, a held `,`); `rejections`, `categories`, `probes` shared with `analyze`, `comment_anchors`, moved verbatim |
| code `ron/structure.rs` | a private `Slot` per open entry, element and root value beside its path segment: a resolved comment takes its id, `value_parsed` records the span, `close_slot`; `Walked.values` (per marker), `.spans` (per id); `Pending.fallback` carries the id |
| code `lib.rs` | root re-exports `RonValues`, `value_hashes` |

No deviation from the spec's names, shapes or recipe. Readings (R2, R2b, R3 owner pre-approved): R1 the span is token indices into the one lex, internal; R2 a comment the walk anchors to a map entry while its key is parsed names the entry's value; R2b one inside the key's elements (`{ (/* m */ 0, 0): 1 }`) names that element; R3 the walk keeps its capped recursion (rule reworded from "an explicit stack": a rewrite risks the 53 × LF/CRLF adjacency results for no criterion).

The test-engineer's L1-L13, adopted (`expected.json` cites them): L1 an entry's value excludes its key and `:`; L2 the root starts after `#![enable(..)]`; L3 the `,` after a value is outside it; L4 CR dropped only before LF; L5 the detail alphabetical (`broken_stray.ron`: `expected_separator`), empty file `empty_file`; L6 no `Path` comment: `Hashed([])`; L7 an inner comment dropped, its own entry; L8 `Player( // m` hashes `Player(..)`; L9 one entry per comment, one digest per value; L10 a pair names a comment by its whole text, the key its full range; L11 two runs: two calls plus another thread; L12 timing 10× `comment_anchors` + 250 ms, best of three; L13 forms struct, named struct, tuple (`Some(..)`, `()`), scalar, nested, `root`.

Tests: 23 new tests green (`hash_levels_ron.rs` 17, eval `hash_levels_ron_fixture.rs` 6), 8/8 mutations killed (M1-M8); one equivalent mutant at `ron/mod.rs:432-437` (a comma still held at the span's end is unreachable on accepted files). R2 pinned at `hash_levels_ron.rs:872`, R2b at `:905`. `import_cli`, `layout_cli` red only until `fixtures/hash-levels/ron/` is committed.
