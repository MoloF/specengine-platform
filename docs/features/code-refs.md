---
class: spec
status: draft
scope: [crates/specengine-code, crates/specengine-core, crates/specengine-cli, crates/specengine-eval, fixtures]
ref: Phase 3 plan 2026-10-10, slice 1 (WA-1..WA-14, RD/TE/SW criteria; Q1, Q2 decided under the owner's pre-approval, review open); 05 s5.3; 07 s2, s5; 08 s2 Phase 3
adrs: []
---

# code-refs: markers bound to code, spec refs

## Why

Nobody sees which code implements, verifies, configures or assumes a node: markers bind only in RON, for the eval alone (`docs/canon/code-identity.md`); code citations are not the weak mentions ADR-0016 promised; a lagging marker (`@2` on rev 3, 05 s5.3) is invisible; agents lack 07 s2's reverse lookup; slices 3-4 need a binding model.

The slice reads a configured code tree and writes nothing: each marker gets one Rust item, module or RON path, or a reason; its ID resolves as `spec show`'s REF; `@N` meets the node's revision; comment citations become weak mentions; `spec refs REF [--json]` lists them. No ADR (ADR-0008, -0009, -0010, -0012, -0016, -0018, -0020, -0026, -0031 applied).

## Description and interactions

- **The owner** adds `[code]`; **an agent** runs `spec refs R-12` before changing a node's code or text.
- **Crates** (WA-12): core gains the pure `[code]` reader; `specengine-code` the Rust binding (three dependencies kept); the CLI `specengine-code` (+ pinned `toml` only for `[package] name`) and `refs`, grammar and resolver staying there; store nothing. The CLI's graph, hence MCP's and the daemon's, gains tree-sitter's crates (`tree-sitter-language`, `streaming-iterator`, `regex`; `cc` builds); CLI README line 11 ("never ... `specengine-code`") changes at shipping.
- **Unchanged** (WA-11, WA-13): `spec check` (no code finding), hook triggers, MCP, HTTP, UI, `INDEX_FORMAT`; no `[code]` here (its doc comments hold marker examples).

## Data

### The code table

WA-1.

```toml
[code]
roots   = ["crates", "assets/data", "build.rs"]        # required, non-empty
exclude = ["**/target/**", "crates/*/tests/fixtures/**"] # optional
```

- Closed: another key (07 s5's `languages`, `crates`, `bevy`, `scip` until their slices), a wrong type, `roots` missing or empty, a path not `is_clean_relative` (one trailing `/` dropped, as `[paths]`), a glob outside the `[paths]` exclude dialect (`docs/canon/gate.md` "Config") -> `specengine.toml:<line>: message`, exit 2, in commands reading code (`refs`); the others only name-check `code`.
- No table: off. No default root (ADR-0008).
- A root is a directory or a `.rs`/`.ron` file; missing -> `note: [code] root <path> does not exist`, another file type -> `note: [code] root <path> is not a .rs or .ron file`; neither contributes.

### The walk

WA-2, WA-3, WA-9.

- Regular `.rs`, `.ron` files under the roots minus `exclude`, as the md walk (store README): symlinks, `.`-names skipped, no `.gitignore`, byte-sorted, repeats dropped, non-UTF-8 names counted.
- Content not UTF-8: `note: <path> is not UTF-8: skipped`, counted. An unlistable directory, an unreadable file, no tree from the parser -> exit 2, `spec: the code walk is incomplete: <gap>` (the first by path, worded as `export index`'s), stdout empty, never a partial list.
- A `.rs` file's package: the nearest directory, its own up to the root, holding a regular `Cargo.toml`; targets by `layout_targets` (`code-identity.md` "Target table"), named by `[package] name`, else the directory; none -> `Unrooted`. No subprocess (`cargo metadata`: slice 6); Cargo's names from `identity_literals.rs`.
- Read: the config, the corpus (every read's index refresh), each walked file once, the probed manifests. **Written: only the data directory**; nothing about code is stored.

### Binding rules

WA-4. *Item*: what `analyze_file` collects (`ITEM_KINDS` at file level and directly in an inline `mod`, `MEMBER_KINDS` directly in an `impl` or `trait`; nothing in function bodies). *Comment*: line or block, doc comments too; a multi-line block comment trails by its `/*` line, leads by its `*/` line (as RON). First rule that applies:

| # | The comment | Binds | Example |
|---|---|---|---|
| 1 | is a block comment and an item starts later on its `*/` line | that item | `/* @implements R-12 */ fn f() {}` |
| 2 | stands on its own line, and the next token, past blank lines, comments and outer attributes `#[..]`, starts an item | that item | `// m`, `#[derive(Debug)]`, `struct S;` |
| 3 | follows an item's last token on its line (several end there: the one right before) | that item | `fn f() {} // m`; `} // m` closing an `impl` |
| 4 | follows an item's opening `{` on its line | that item | `impl S { // m` |
| 5 | is an inner doc comment (`//!`, `/*!`) at file level or directly in an inline `mod` | the file's module or that `mod` | `//! m` in `src/a.rs` -> `<pkg>::a` |
| 6 | anything else | `unanchored` + category | below |
| 7 | the item of 1-5 has `has_error()` (never a file module) | `cannot_verify` + the hash's `ErrorCategory` | an `fn` missing its `)` |

| Category, first that applies | The comment is | Example |
|---|---|---|
| `in_macro` | in a macro call's token tree or a `macro_rules!` body | `foo! { // m` |
| `in_body` | in a function body, block or initializer | `let x = 1; // m` |
| `no_item` | followed, past what rule 2 crosses, by `}` or the file's end | the last line of a file |
| `not_an_item` | at a `use`, `extern crate`, item-position macro call, field, variant, associated `const`/`type`, `#![..]` | `// m` above `use a::b;` |

Strict first: fields and variants bind nothing yet. A bound item gives its `qpath` (`code-identity.md` "Units and `qpath`"), `kind()`, first line, ambiguity (`path_attribute`, `unrooted`, `duplicate`: shown, not resolved). RON anchors stay: a path, `duplicate` ("RON ambiguity"), `unanchored` `no_value`, `cannot_verify` `nesting_too_deep`. Linear; LF, CRLF alike.

### States

WA-5, WA-6, WA-10. A marker's **token**: its ID's first byte to the first whitespace, `[`, `*/` or line end (`R-12@3`, `slug/AC-07`, `MEC-A#RULE-B@2`). The ID is read by `grammar::parse_reference` and resolved by `spec show`'s resolver (CLI README "Rules"); `#` stops being non-Latin (`markers.rs` `is_latin_id`).

| `id_state` | Meaning | Listed under |
|---|---|---|
| `resolved` | one node by its `id` (`slug/ID`, `ID#SECTION` too) | that node |
| `alias` | one node via `aliases_from` or `aliases:` | that node |
| `ambiguous` | several nodes (duplicates, several holders) | each |
| `homoglyph` | look-alike or mixed script, Latin `fix` shown, never resolved (ADR-0009) | the fix's node |
| `other_project` | a `project:` qualifier (Phase 5) | none |
| `queue` | no node, and the queue's form `PR-` + digits (corpus first: a project prefix `PR` resolves); not looked up (slice 9) | none |
| `dangling` | else: empty ID, no reference, unknown prefix, no node | none |

A node's **revision** R: a document's front-matter `rev`, a section's own `rev=N` (never its document's; core README Q3, a working answer until slice 2's ADR); none written -> 1 (Q1). The marker's `@N`, first that applies:

| `rev_state` | When |
|---|---|
| `unversioned` | no `@` after the ID |
| `bad_rev` | `@` not followed by 1-9 ASCII digits ending the token: `X@`, `X@3a` (today read as 3), `X@1234567890` |
| `rev_unknown` | `id_state` not `resolved`, `alias`, `ambiguous`; or R written but unreadable |
| `current` | N = R |
| `spec_ahead` | N < R: the marker was not raised |
| `predated` | N > R: no such revision |

`ambiguous` compares per node; `@assumes` is judged like the others.

### Mentions

WA-7, Q2. `grammar::scan` over every Rust and RON comment, doc comments too; never string literals or code. The hit at a marker's own token is skipped, other hits count. A mention gets an `id_state` (no `rev_state`) and belongs to the binding its comment would get as a marker; if unanchored, the innermost enclosing item; else the file. Weak (`MENTIONS`, model README "Links").

### spec refs

WA-8. REF: every `spec show` form (`@rev` ignored with show's `note:`; look-alike, mixed script, `project:` -> exit 2; several holders -> show's `warning:`, a block each). A block lists what names its node (table above), not its sections or parents. Order of work: config, REF, walk.

```
R-12 | requirement | Regeneration | docs/spec/stamina.md:9 | rev 3
verifies | crates/a/tests/regen.rs:5:4 | R-12@2 | resolved | spec_ahead | [body] | item crates/a::test:regen::zero function_item | mutation: drop the clamp
mention | crates/a/src/hud.rs:40:12 | R-12 | resolved | file
refs: markers 1, mentions 1; code files 41 (.rs 38, .ron 3), skipped 0
```

- Header: search's first four fields + ` | rev <R>` (`rev 1 (none written)`, `rev unknown`); blocks apart by an empty line.
- Marker: `<relation> | <path>:<line>:<column> | <token> | <id_state>[ <fix>] | <rev_state> | <levels> | <binding>[ | <note>]`, 1-based, column in bytes at the `@`; levels `default`, `[a, b]`, `invalid:<category>`.
- Mention: `mention | <path>:<line>:<column> | <reference as written> | <id_state>[ <fix>] | <binding>`.
- Binding: `item <qpath> <kind>[ ambiguous <reason>]`, `module <qpath>[ ambiguous unrooted]`, `ron <path>#<field path>[ ambiguous duplicate]`, `unanchored <category>`, `cannot_verify <category>`, `file` (mentions).
- Rows: markers by relation (`implements`, `verifies`, `configures`, `assumes`), path bytes, line, column; then mentions by path, line, column; the summary last, counting all.
- Cap: `OUTPUT_CAP_CHARS` (CLI README "Output and the cap"): rows while within it, then `[truncated: <k> markers, <j> mentions not shown]`.

`--json`, the text's rows, every key present (absent `null`), in this order:

```json
{"ref":"R-12","reason":null,"notes":[],"nodes":[{"id":"R-12","kind":"requirement","title":"Regeneration","path":"docs/spec/stamina.md","line":9,"rev":3,"rev_written":true,
 "markers":[{"relation":"verifies","path":"crates/a/tests/regen.rs","line":5,"column":4,"token":"R-12@2","id":"R-12","rev":2,"id_state":"resolved","fix":null,
  "rev_state":"spec_ahead","levels":["body"],"levels_state":"declared","note":"mutation: drop the clamp",
  "binding":{"kind":"item","qpath":"crates/a::test:regen::zero","item_kind":"function_item","line":6,"ambiguity":null,"category":null}}],
 "mentions":[{"path":"crates/a/src/hud.rs","line":40,"column":12,"written":"R-12","id_state":"resolved","fix":null,"binding":{"kind":"file",...}}]}],
 "code":{"files":41,"rs":38,"ron":3,"skipped":0},"omitted":{"markers":0,"mentions":0}}
```

`binding` always has its six keys, `kind` one of the binding words above; node `rev` `null` when unknown; `levels`, `levels_state` as the eval's `markers.json` (`code-identity.md` "Eval outputs").

**Exit**: 0 answered, zero rows too. 1 REF names nothing (show's `spec: <reason>`; JSON `nodes` `[]`, `code`, `omitted` `null`). 2, stdout empty: usage; discovery; config; no table, exactly `spec: specengine.toml has no [code] table: the code layer is off (add [code] with roots)`; REF as above; a walk gap.

## Rules and edge cases

- WHEN one state is read twice the system SHALL print byte-identical stdout, stderr and JSON, whatever the absolute root, cwd, creation order, LF or CRLF.
- Nothing blocks (ADR-0012): every state is listed, never refused. One row per marker or citation; short prefixes in prose yield mentions (the grammar decides).

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- `languages`, glob `a/[b]`, `roots = []`, no `roots`, `roots = ["../x"]` -> exit 2, `specengine.toml:<line>:` at the key. M: accept an unknown key -> `code_table_unknown_key_is_refused`.
- [ ] RD-2 -- the walk. M: follow a symlink -> `code_walk_skips_symlinks`; print before the gap -> `code_walk_gap_is_exit_2`.
- [ ] RD-3 -- binding rules in `specengine-code`, no new dependency. M: cross a `use` -> `marker_above_use_is_unanchored`; stop at an attribute -> `marker_above_derive_binds_the_struct`; trailing to the next item -> `trailing_marker_binds_its_line_item`; body to its `fn` -> `marker_in_body_is_unanchored`; drop `has_error` -> `marker_on_broken_item_cannot_verify`.
- [ ] RD-4 -- states. M: the document's rev for a section -> `section_marker_compares_the_section_rev`; `>=` for `current` -> `newer_marker_is_predated`; resolve a look-alike -> `homoglyph_marker_is_never_resolved`.
- [ ] RD-5 -- mentions. M: scan strings -> `string_literal_ids_are_not_mentions`; count the marker's token -> `marker_id_is_not_a_mention`; a doc-comment mention to the file -> `doc_comment_mention_belongs_to_its_item`.
- [ ] RD-6 -- `refs`. M: exit 0 without `[code]` -> `refs_without_code_table_is_exit_2`; discovery order -> `refs_is_byte_identical_across_roots_and_order`.
- [ ] RD-7 -- read-only. M: a cache file under the root -> `refs_leaves_the_worktree_untouched`.
- [ ] RD-8 -- WA-12's dependencies. M: `specengine-code` in the store -> `store_depends_on_model_core_and_sqlite_but_no_measurement_crate`; tree-sitter in the core -> new `core_and_store_have_no_tree_sitter`.
- [ ] RD-9 -- optional: TE-9's counts in "Implementation".

test-engineer:

- [ ] TE-1 -- new `fixtures/code-a/` + `expected.json` (`spec-a`, `format_history.txt` untouched): a document `rev: 3`, a section `rev=2`, a node without rev, an `aliases_from` alias, a feature-scoped AC; `.rs` (lib, `tests/`, a parse error), `.ron`.
- [ ] TE-2 -- every binding rule and category a row, LF and CRLF (built at test time), each naming its mutation.
- [ ] TE-3 -- every `id_state`, `rev_state` (via the library where `refs` lists none); U+0420 for `P`; `X@3a`; `@1` on a node without rev -> `current`.
- [ ] TE-4 -- two roots, another cwd, reverse creation: byte-identical stdout, stderr, JSON.
- [ ] TE-5 -- in a temp git repository `refs` leaves `git status --porcelain` empty, code files' (path, size, mtime, BLAKE3) unchanged.
- [ ] TE-6 -- `refs` on `fixtures/spec-c` exits 2 naming `[code]`; the new modules hold no fixture prefix, project path, kind or stack word.
- [ ] TE-7 -- `spec check` on `code-a` with dangling markers equals it with the code removed. M: check reading code.
- [ ] TE-8 -- `build_graph.rs`: `CLI_FORBIDDEN` loses only `specengine-code`; `cli_direct_dependencies_are_unchanged_by_w`, `store_and_cli_normal_dependencies_are_2a1_s` gain it (+ `toml`); `core_and_store_have_no_tree_sitter`; licences green; a mutation per changed assertion.
- [ ] TE-9 -- pilots, manual (`#[ignore]`, release, `--timeout`, foreground): config copies outside the repository plus a code table, trees read-only (eval README's proof); counts by state, wall time (AC-10 baseline); no pilot name or path committed.

spec-writer:

- [x] SW-1 -- this spec.
- [ ] SW-2 -- at shipping: new `docs/canon/code-bindings.md` (Tier 2, <= 12 288 B); `code-identity.md` "Open"; CLI README line 11 and `refs`; code README "Modules"; `stack.md`; `spec-check.md`; model README `is_latin_id`; 07 s2, 08 Phase 3; 05 s5.3 a pointer; `CLAUDE.md`.
- [ ] SW-3 -- no ADR; `export index`, `check` clean.
- [ ] SW-4 -- code comments cite this spec's headings, the canon's at shipping (`doc_pointers.rs`).

## Risks

- An intended graph change; lands after `ui-embed` (both edit `Cargo.lock`, `build_graph.rs`). The whole tree is parsed per call.
- qpath ambiguity shown, not fixed (0.2 % on pilot A, `code-identity.md` "Measured").
- Assumed: pilots hold legacy citations, few `@` markers; tree-sitter's crates licensed within the allowed set.

## Out of scope

Hashes, lock, sync, verify, bump, fingerprints (levels echoed); DB tables, `symbols`, `unmapped`, `find_symbols`, `show --bindings`, MCP, HTTP, UI; `cargo metadata`; layers B, C, Bevy; `@assumes` vs the queue; `unbound`; Health; check findings; the gate's file -> node map; writing markers (ADR-0016); `[code]` here; spec-side referrers (`show --links`).

## Open

| Q | Question | Answer |
|---|---|---|
| Q1 | a node without `rev` | revision 1 (tracey's; slice 2's ADR records it) |
| Q2 | mention sources | comments only, never string literals |

Both decided under the owner's pre-approval; the owner's review is open. WA-1..WA-14: the analysis' working answers (2026-10-10), marked where they apply (WA-14: TE-9).

## Implementation

Not started. To record: modules, the crates the lock gained, TE-9's figures, deviations with reasons.
