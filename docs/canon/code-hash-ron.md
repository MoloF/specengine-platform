---
class: canon
tier: 2
scope: [crates/specengine-code, crates/specengine-eval]
owner: owner
reviewed: 2026-10-16
---

# Code hash: the RON value

The hash of the value a RON comment anchors (`code-identity.md` "RON binding"): what judges a `ron` binding from 5b-1 on (`lock-levels.md` "Format 2"), in place of format 1's `file_blob`. v3's header, `put` and CR rule (`code-hash.md` "Levels"), its own tag. Nothing records it yet; 5a-2 measures it. A recorded digest changes only with `LEVEL_RECIPE` and a rebase of every lock (ADR-0050).

## Entry point

`ron::value_hashes(source: &str) -> RonValues`, `#[must_use]`, re-exported at the crate root. One lex and one structure walk per file (one probe per comment, as `comment_anchors`); each value hashed once, however many comments name it.

| `RonValues` | When |
|---|---|
| `Hashed(Vec<(Range<usize>, Digest)>)` | `analyze` accepts the file: one entry per comment whose anchor is `Path`, marked or not, keyed by its range as `CommentAnchor::comment` gives it (a CRLF line comment's CR included), in source order. `Ambiguous`, `Unanchored`, `CannotVerify` anchors get none; no `Path` comment: `Hashed([])` |
| `CannotVerify(&'static str)` | `analyze` rejects the file (`has_error`: a lexer or structure error, `nesting_too_deep` and `empty_file` included): every value refused, as an error can shift a value's span, a false `ok` (08 AC-14). The detail is `error_categories[0]`, the alphabetically first category, not the first by offset |

The span stays inside `ron` (token indices into the file's one lex): no field on `Anchor`, `RonMarker`, `CommentAnchor` or `RonAnalysis`. A caller pairs a digest with an anchor by the comment's range.

## RON value

    value_hash = BLAKE3( level_header() ‖ put("ron") ‖ put(cr(text(t))) … )

for each lexer token `t` of the anchored value, its first through its last:

- **Span**: an entry's value after its `:` (field name or map key and `:` outside); an element; the root value after `#![enable(..)]` and the comments before it. A container runs through its closer, a struct name included (`player: Player( // m` hashes the whole `Player(..)`); the `,` after a value is outside.
- **Drops**: comments (one inside a value that anchors a path gets its own entry); a `,` whose next non-comment token is a closer. Every other token's text verbatim but for the CR rule: a CR right before an LF dropped (only a string holds one; a lone CR is text).
- **Which value**: the one the comment's `Path` names (rules 1-5). A comment the walk anchors to a map entry while its key is parsed names the entry's value, not the key; one inside the key's own elements (`{ (/* m */ 0, 0): 1 }`, path `root{…}.0`) names that element.
- **Levels**: `sig` = `body` = `value_hash`; `deps` = the empty set's digest, `level_header() ‖ put("deps")`, an item's `deps` with no call (crate-private `levels::deps_digest`: 5b-1 decides its public form); no fingerprint, never followed.

Example:

    (
        // m
        b: [2, /* x */ 3,],
    )

`// m` anchors `root.b` (rule 4): after the tag, `put("[") ‖ put("2") ‖ put(",") ‖ put("3") ‖ put("]")`, `/* x */` and the `,` before `]` dropped. `/* x */` anchors `root.b[1]`: its own entry, the stream of `3`.

## Pins and cost

- The RON lexer is outside `code_recipe` (`lock-levels.md` "Risks"): `ron_value_stream_is_pinned` (code `tests/hash_levels_ron.rs`) catches a stream change, which bumps `LEVEL_RECIPE`.
- The value pass is one flat loop over a token slice and adds no recursion. The walk is `analyze`'s, unchanged: one call per container, capped at `MAX_DEPTH` 512 within a 2 MB stack (`ron_values_at_max_depth_hash_on_a_two_megabyte_stack`).
- No path, line, offset or time enters a stream: equal bytes, equal digests, in any run or file order.

## Known limits

- Nested anchored values overlap: O(n × depth), capped by `MAX_DEPTH`.
- One rejection refuses every value of the file (08 AC-14, by design).
- The key-comment choices above, like every stream byte, freeze with 5b-1's first lock.
- The pilots hold no RON marker: bound values are proven on fixtures (`fixtures/hash-levels/ron/`); 5a-2 measures every `Path`-anchored comment.
- `analyze`'s own RON limits apply (`crates/specengine-code/README.md` "Known limits and open minors").
