---
class: canon
tier: 1
scope: [crates/specengine-code]
owner: owner
reviewed: 2026-09-29
---

# specengine-code — layer A: Rust and RON parsing, AST hash, markers, Bevy detector

Layer A of 05 §5.1: tree-sitter only — no build, no name resolution. A default workspace member in the core graph: it never depends on `ra_ap_*`, `syn` or Bevy (`specengine-eval` `tests/build_graph.rs` asserts it). Pins: `tree-sitter =0.27.0`, `tree-sitter-rust =0.24.2` (ABI 15), `blake3`. Today its caller is `specengine-eval`; the Phase 1+ index and `spec verify` build on it.

## Modules

| Module | What it does |
|---|---|
| `grammar` | `TREE_SITTER_VERSION`, `TREE_SITTER_RUST_VERSION` (a test checks them against the built artefact), `grammar_info()` (ABI), `RustParser` |
| `hash` | recipe `specengine-hash/v2` of 05 §5.2: `RECIPE` header, iterative walk, comments filtered by `kind()` (never `is_extra()`), anonymous `,` skipped, u32-LE length prefixes, the attached attribute run hashed before the item, normalisations N1–N4. `hash_item` → `HashState`: a `Digest`, or `cannot_verify` with an `ErrorCategory` for an item with `has_error()` or a `use` run nested past `MAX_USE_RUN_NESTING = 64` (`nesting_too_deep`) — never hashed. `normalize` returns `false` for an incomplete stream |
| `comments` | `comment_ranges`, `strip_comments` (`///` included) — the comment perturbation |
| `items` | `analyze_file` / `analyze_tree` → `FileAnalysis`: items (`ITEM_KINDS`; members `MEMBER_KINDS`) with their hash state, `mod` declarations, `#[path]` attributes; collected on an explicit stack |
| `qpath` | `qpath`, `file_role` (`FileRole`: `CrateRoot`, `Module`, `Unrooted`), `resolve_path_attribute`; one `Ambiguity` per item (`PathAttribute`, `Unrooted`, `Duplicate`), reported, never guessed |
| `markers` | `markers_in`: `Relation` (`implements`, `verifies`, `configures`, `assumes`) and `Marker` (ID, optional rev, note, `id_latin`); several per comment, shared by `.rs` and `.ron`; a non-Latin ID clears `id_latin`, never fatal (ADR-0009) |
| `ron` (`mod`, `lexer`, `structure`) | own RON lexer (`lex`: comments, `#![enable(..)]`, raw strings, `LexError` categories) and a tolerant structure walk. `analyze` → `RonAnalysis`: every marker with an `Anchor` (`Path` such as `root.player.speed`, `Unanchored`, `CannotVerify`), `Rejected` categories, comment byte ranges. Binding rules 1–5, segment syntax and the caps `MAX_SEGMENT_BYTES = 128`, `MAX_DEPTH = 512`: 05 §5.3; every rule with examples: module doc of `ron/structure.rs` |
| `bevy` (`mod`, `expr`, `tokens`) | syntactic registration detector (05 §5.1): `detect` / `detect_file` → `BevyAnalysis` — systems of `add_systems` (tuples on an explicit stack to `MAX_NESTING = 256`, `COMBINATORS` peeled, `ADAPTERS` recorded), observers, plugins (`impl Plugin for` and `fn(&mut App)`, `PluginKind`), plugin uses, `Uncertain` with an `UncertainCategory`. `expr` reads ordinary code exactly from the syntax tree; `tokens` reads `macro_rules!` transcribers and macro arguments from flattened tokens, names only. Texts capped at `MAX_TEXT_BYTES = 128` plus `…` |

## Rules

- What cannot be read is `cannot_verify`, `uncertain` or `unanchored` with a category — never a guessed name and never a shared hash (trap 1, 05 §5.2).
- Only `kind()` strings, never `kind_id()`. A grammar bump changes the recipe header and is re-measured with `specengine-eval ast-hash` and `ron` before it lands.
- Cost is linear in the input and nothing recurses over it; the caps above bound crafted input.
- No subject domain (ADR-0008): RON and Bevy are language and framework support; project specifics arrive from the caller.

## Known limits and open minors

- **Marker grammar gap (Phase 1).** The canon form is `// @implements ID@rev [tiers]` (`docs/canon/architecture.md#markers`, 05 §5.3). `markers_in` does not parse the `[tiers]` list yet: everything after `ID[@rev]` is kept as the note.
- **`qpath` target discriminator (Phase 1).** `src/bin`, `examples` and `tests` targets share an empty root module path: 16.5–32.2 % of pilot items were `Duplicate`.
- Hash: N3 hashes token-level DSL macros the same with and without their significant `| {` / `=> {` braces (remedy: a per-project opt-out list of macro names in `specengine.toml`, not built); a nested brace-delimited macro inside an expression macro inherits N3; `mod x;` and `extern crate` runs reordered by rustfmt are not sorted (v3 candidate).
- RON: two keys sharing their first 128 bytes render one path (Phase 1 flags such paths as ambiguous, never merges them); markers pending when error recovery consumes the next token are `Unanchored` (or their trailing fallback), not `CannotVerify`; crafted input costs more than linear in three places — an unterminated raw string with thousands of `#`, tens of thousands of markers on one comment line, paths up to ≈ 512 × 136 bytes per marker at maximal nesting.
- Bevy token reader (macro bodies and arguments only; pinned by tests): a comma inside a type position or closure parameters splits an element — `|q: Query<A, B>| …` as a whole argument gives `arguments` uncertain, and with ≥ 3 generic items a middle path fragment can be registered as a name; `a::<` unclosed at an element's end reads as `a`; tokens after a path or postfix chain → `expression`; `sys.$m(c)` and `systems::$name` → `expression` (should be `metavariable`); `::<<` opens a turbofish; `plugin_uses` names `P::<T>::default()` as `">"` and has no leftover rule; its `piped` list differs from the code reader's.
- Bevy output is sorted by line only: calls of one chain on one line come out reversed, while the module doc claims source order.
- Invisible by design: macro-generated items, `inventory` / `linkme` registration, generic instances of one system (layer B or C, 05 §5.1).

## Tests

`tests/{hash_traps,hash_v2,hash_depth}.rs`; `tests/{ron_markers,ron_depth,ron_cost}.rs` (`ADJACENCY`: 53 cases × LF / CRLF, each adjacency rule with a named mutation); `tests/bevy_detector.rs`. Fixtures `fixtures/{ast-hash,ron,bevy-mini}`, each with `expected.json`; `bevy-mini` is a workspace-excluded Cargo project, only parsed. Run one file: `cargo nextest run -p specengine-code --test <file>`.
