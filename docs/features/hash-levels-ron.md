---
class: spec
status: draft
scope: [crates/specengine-code, fixtures]
ref: Phase 3 slice 5a-1b, split from hash-levels 2026-10-15 (its Q12, decided under the owner's pre-approval, review open; WA-3; Q7, Q15); 05 s5.2; 08 s2, AC-14
adrs: []
---

# hash-levels-ron: the value hash of an anchored RON value

## Why

A format-1 `ron` binding is judged by `file_blob` alone: any byte of its file and an autocrlf checkout stale it (`docs/canon/spec-lock.md` "Lockability"; `spec-verify.md` "Known limits"). 5b judges it by the anchored value's hash instead (`lock-levels.md` "Format 2": `ron` sig = body = `value_hash`). Once a lock records that hash, its bytes change only with a `LEVEL_RECIPE` bump and a `spec lock rebase` of every lock (ADR-0050), so they are fixed and pinned before 5b.

Split from 5a-1 (`hash-levels.md` Q12) so a third iteration on one slice cannot leave the Rust levels unshipped. Shares only `level_header()`, `put` and the CR rule with 5a-1 (`hash-levels.md` "Levels"), shipped there first. Order (08 s2): 5a-1 -> 5a-1b -> 5a-2. No ADR.

## Description and interactions

- **Nobody sees it yet**: no command, MCP tool, lock field or UI. 5a-2 measures it (`hash-levels-eval.md` "`ron`: `result.value_hash`"), 5b records and judges it (`lock-levels.md` "Format 2", "Level-aware verify" step 3; `lock-rebase.md` "`spec lock rebase`" **Location**).
- **Genre** (ADR-0008): the RON lexer and structure walk stay in `specengine-code` `ron`; core sees digests only.
- **Anchors unchanged** (Q15): `analyze`, `comment_anchors`, `Anchor`, `RonMarker`, `CommentAnchor`, `RonAnalysis` keep their shape and results. A caller pairs a value hash with an anchor by the comment's range: the CLI already holds `CommentAnchor`s (cli `code_walk.rs` `RonFile`).
- **Crates**: code (`ron/structure.rs` carries the value ranges, internal; `ron::value_hashes` and `RonValues` in `ron/mod.rs`; `lib.rs` export); `fixtures/hash-levels/` (`.ron` inputs). Core, store, CLI, eval untouched; no new dependency.

## Data

### Entry point

Digests are `hash.rs` `Digest`, exported at the crate root beside 5a-1's entry points.

| Name | Shape |
|---|---|
| `ron::value_hashes(source: &str) -> RonValues` | "RON value": `Hashed(Vec<(Range<usize>, Digest)>)`, one per comment with a `Path` anchor, marked or not (5a-2 measures unmarked ones: the pilots hold no marker), keyed by its range as `comment_anchors` gives it (`CommentAnchor::comment`), in source order; or `CannotVerify(&'static str)`; one lex and one structure walk per file |

### RON value

WA-3; closes `code-identity.md` "Open" (level meaning in RON). Per comment whose anchor is `Path`: `value_hash` = BLAKE3(`level_header() ‖ put("ron") ‖ put(text(t))` for each lexer token `t` of the anchored value, from its first token through its last: a container through its closer, a struct name included; comments dropped; a `,` whose next non-comment token is a closer dropped; every other token's text verbatim but for the CR rule). `sig` = `body` = that digest; `deps` = the empty set's digest (`level_header() ‖ put("deps")`, an item's `deps` with no call); no fingerprint (RON is never followed).

Example:

```ron
(
    // m
    b: [2, /* x */ 3,],
)
```

`// m` anchors `root.b` (rule 4, `code-identity.md` "RON binding"); its entry is keyed by the comment's byte range, its stream `level_header() ‖ put("ron") ‖ put("[") ‖ put("2") ‖ put(",") ‖ put("3") ‖ put("]")` (`/* x */` and the `,` before `]` dropped). `/* x */` anchors `root.b[1]`: its own entry, the stream of `3`.

- **Refusal** (Q7): any `RonAnalysis` rejection (`has_error`: a lexer error or a structure error, `nesting_too_deep` included) refuses every value of the file: `CannotVerify(<the first error_categories entry>)`; a structure error can shift a value's range, a false `ok` (AC-14). `Ambiguous`, `Unanchored`, `CannotVerify` anchors get no entry.
- **Range** (Q15): the structure walk carries each anchored value's byte range beside its path (today only the path: `ron/mod.rs` `Anchor`) inside `ron` alone: no public field is added to `Anchor` (tests build `Anchor::Path` literally: `ron_ambiguity.rs`, `ron_depth.rs`), `RonMarker` or `CommentAnchor` (the CLI holds `CommentAnchor`s; the eval reads `RonAnalysis::markers`); `value_hashes` is the range's one way out, keyed by the comment's range.
- The lexer is outside `code_recipe` (`lock-levels.md` "Risks"): `ron_value_stream_is_pinned` catches a change, which bumps `LEVEL_RECIPE`.

## Rules and edge cases

- WHEN the same bytes are hashed in two runs or in reverse file order, the system SHALL give the same digests: no path, line, offset or time enters a stream.
- One lex and one structure walk per file, on an explicit stack; never a re-lex per comment.
- WHEN `analyze` or `comment_anchors` runs on any source, the system SHALL give what it gave before this slice: the shipped `ron_*` tests of `specengine-code` pass unedited.
- Code comments cite `crates/specengine-code/README.md` "Hash recipe", never this spec's headings (compacted at shipping), as 5a-1's (`hash-levels.md` "Rules and edge cases").

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- "RON value". M: comments hashed -> `ron_comment_edit_keeps_the_value_hash`; sibling tokens -> `ron_sibling_edit_keeps_the_value_hash`; trailing comma kept -> `ron_trailing_comma_keeps_the_value_hash`; a lexer error hashed -> `broken_ron_file_value_cannot_verify`; a structure error hashed -> `ron_structure_error_refuses_every_value`; CR kept -> `ron_value_hash_equal_under_lf_and_crlf`; lexer drift -> `ron_value_stream_is_pinned`.
- [ ] RD-2 -- "Entry point". M: a re-lex per comment -> `ron_value_hashes_linear_in_comments` (best of three, as `ron_cost.rs`); markers only, or a key other than `CommentAnchor::comment` -> `unmarked_path_comments_are_keyed_as_comment_anchors` (SW refinement). The code-reviewer: no public field added to `Anchor`, `RonMarker`, `CommentAnchor` (Q15).

test-engineer:

- [ ] TE-1 -- `.ron` inputs in `fixtures/hash-levels/`, its `expected.json` extended: per value form (scalar, multi-line string, struct, named struct, tuple, list, map, nested) an equal pair and a different pair, LF and an in-memory CRLF copy, and a file `RonAnalysis` rejects (a lexer error; a structure error) refusing every value, each pair naming its mutation.
- [ ] TE-2 -- determinism: two runs, reversed file order.

spec-writer:

- [x] SW-1 -- this spec (split from `hash-levels.md`, 2026-10-15, its Q12).
- [ ] SW-2 -- at shipping: "RON value" in `docs/canon/code-hash.md` (5a-1's) after "Fingerprints" when it holds (≤ 12 288 B, measured), else a Tier 2 `docs/canon/code-hash-ron.md` (`docs/index-canon.md` re-measured); `code-identity.md` "Open" loses "level meaning in RON"; the code README's `ron` row names `value_hashes` and that section (net ≤ 0); 08 s2's slice list; `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- **Frozen bytes**: as 5a-1's (`hash-levels.md` "Risks"): once 5b writes locks, a lexer or stream change costs a `LEVEL_RECIPE` bump, a rebase of every lock and a re-key of every open drift item.
- **RON cost**: per-comment re-lexing is quadratic (hence one lex per file); nested anchored values (every `Path` anchor, marked or not) cost O(n × depth), capped by `MAX_DEPTH` 512.
- **Pilots hold no marker**: 5a-2 measures every `Path`-anchored comment and the refusal (`hash-levels-eval.md` Q1); bound values are proven on fixtures only.

## Out of scope

The Rust levels and fingerprints (5a-1, `hash-levels.md`); the eval and pilot figures (5a-2, `hash-levels-eval.md`); CLI, lock, verify, ADR; a public range field (Q15); a RON fingerprint, following RON or a RON shape level (`anchor-states.md` "Out of scope"); the RON grammar itself (`code-identity.md` "RON binding").

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q7 scope of the RON refusal | any `RonAnalysis` rejection refuses every value in the file; the detail is the first `error_categories` entry (5b's `ron_lexer` amended) | lexer errors only: a structure error can shift a value's range, a false `ok` (AC-14) |
| Q15 the value ranges | internal to `ron`, no public field on `Anchor`, `RonMarker` or `CommentAnchor`; `value_hashes` the one way out | a field on `RonMarker` or `CommentAnchor` breaks struct literals; "CLI untouched" no longer certain |

Q-numbers are shared with `hash-levels.md`, where Q9 (the CR rule) and Q12 (this split) live. Q7 decided 2026-10-13, Q15 decided 2026-10-15, both under the owner's pre-approval, review open; WA-3: the 5a analysis' working answer (2026-10-10); "SW refinement": the spec-writer's.

## Implementation

Not started. Split from `hash-levels.md` (2026-10-15, Q12): its RD-5 -> RD-1; RD-9's RON test, renamed `ron_value_hashes_linear_in_comments` -> RD-2 (plus `unmarked_path_comments_are_keyed_as_comment_anchors`); TE-1's RON values -> TE-1 (plus the refused files); TE-4's RON part -> TE-2. To record: modules, deviations with reasons.
