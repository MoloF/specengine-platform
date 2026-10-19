---
class: spec
status: draft
scope: [crates/specengine-eval, crates/specengine-code, fixtures]
ref: Phase 3 plan 2026-10-10, slice 12a of 12 machine-bindings (split 12a/12b/12c; Q1-Q11 decided under the owner's pre-approval, review open; 12b, 12c not drafted); build order 2026-10-16 (08 s2); refreshed 2026-10-19 against shipped 6a, 6b-1, 8a-1, 10a (S1-S16; D1-D9 decided under the owner's pre-approval, review open); 05 s5.1, s5.5; 08 s2 Phase 3, s4.1
adrs: []
---

# constant-census (12a): which string constants carry a record's code

## Why

An agent changing a check needs the record defining the code, the constant carrying it, where it is emitted and the tests asserting it. Today it greps, and coverage stays invisible: which codes have no constant, no test, no record. 08 §2 Phase 3 wants this `check → const → test` chain derived, not hand-marked; 08 §4.1 has the importer carry check codes over.

Slice 12 is split (Q9). **12a** (this): a read-only `specengine-eval` report of which free string constants of a project's code carry a code of its records; no ADR (ADR-0008, -0016, -0020 applied: it binds, locks and writes nothing). **12b** `machine-bindings` and **12c** `constant-reach` are not drafted: no user exists yet (WA-10, WA-11; "Later").

## Description and interactions

- The owner or an agent preparing a consumer project's migration runs `specengine-eval constants` beside `census` and `import` (08 §4.2.1 dry run).
- `specengine-code` gains the value reader, a new module (D4); `items.rs`, `hash.rs`, `levels.rs`, `ron/` keep their bytes (RD-7). `specengine-eval` gains the `constants` module and its wiring: records through `specengine-import` (`import::run`), files by `harness::rust_files`' rules (D1), items and markers through `specengine-code` (`RustParser`, `bind_tree`, `markers_in`, D6). All existing dependencies, none added; `specengine-import` keeps neither `tree-sitter` nor `specengine-code` (its `Cargo.toml`; RD-5 pins it).
- Unchanged: import's outputs and stdout keys, every `spec` command, the code index and its stamp, core, store, CLI, MCP, HTTP, UI, plugin, hook, CI. Nothing writes code, a marker, the corpus or the queue (WA-6).
- Canon at shipping (D8): a new Tier 2 `docs/canon/code-constants.md` takes "Data" and "Rules"; the eval and code README rows point to it (SW-2).

## Data

### Invocation

```
specengine-eval constants [--pilot <dir>] --out <scratch-dir> [--label <name>] [--config <toml>] [--timeout <s>]
```

Flags, labels, the `--out` guard, `--timeout` and exits as `census` (eval README "CLI contract"): `--config` the census config, default the corpus root's `census.toml`, for `--label pilot-a` / `pilot-b` a set `SPECENGINE_CENSUS_CONFIG_A` / `_B` first; no `--scheme`. Fixture (no `--pilot`): `fixtures/code-consts/one`; the other convention by `--pilot fixtures/code-consts/two`. Also exit 2, nothing written: a config with no `[code] roots` (`<config>: [code] roots is empty`; SW refinement).

### Inputs

- **Files** (D1): each census `[code] roots` entry (corpus-relative): a directory listed by `harness::rust_files`' rules (sorted; hidden directories, names starting `target`, symlinks skipped; an unreadable subdirectory on stderr, skipped), a `.rs` file root kept as is; minus a path any `[code] exclude` glob matches (the corpus-relative, `/`-separated path, as `CodeConfig::keeps`); `extensions`, `strip` unused, `.rs` only, `.ron` never; a file under two roots read once. A root that cannot be read: `roots_missing`. A file not UTF-8, unreadable or given no tree: `skipped`.
- **Items** (D6): per file one parse, `bind_tree(&tree, source)`: its `analysis.items` of kind `const_item` or `static_item`, unhashed (`code-bindings.md` "Rust binding": file level and inline `mod`s; never function, `impl` or `trait` bodies).
- **Markers** (D6): the same binding's `comments` whose `binding` is `Binding::Item(i)`; each marker `markers_in` finds there, any relation, `Marker.id` verbatim (no ID resolved: no index).
- **Records** (D7): `import::run(corpus, config)`: each `Import.records` entry's `id` and `aliases` (`[ids] legacy`), each `Import.duplicates` entry's `id`; the `[ids] hyphenless` `Pattern.regex`es.

### Reader (D4)

`specengine-code` `const_value.rs`, exported from `lib.rs`:

```rust
pub fn const_value(tree: &Tree, source: &str, item: &ItemRecord) -> Option<ConstValue>;
pub enum ConstValue { Valued(String), NotValued(NotValued) }
pub enum NotValued { Broken, Macro, ByteString, Escaped, MultiLine, Other } // as_str(): the names below
```

`None` unless `item.kind` is `const_item` or `static_item`; its node is the one spanning `item.byte_range`. It reads the initializer node and its literal's bytes, never below: linear, no recursion (code README "Rules"); no I/O, no hash, nothing stored, so `CODE_INDEX_FORMAT` stays 4.

### Value (WA-2, D2)

The text between the quotes of an item's whole initializer when that is one plain `"…"` or raw `r#*"…"#*` string literal, never unescaped. Else `not_valued`, the first category that applies: `broken` (`has_error`); `macro` (a macro invocation, `concat!`, `include_str!`, or any call); `byte_string` (`b"…"`, `br"…"`, `c"…"`, `cr"…"`, a char); `escaped` (a plain literal holding `\`); `multi_line` (a literal holding LF or CR); `other` (no initializer, a number, a path, `&[..]`, parentheses, a cast, a block). An associated const is no item, so never read (D2).

### Match (WA-3, D7)

Whole value, byte for byte (no prefix, substring, case fold or trim); each valued item takes the first that applies:

1. `ambiguous` `two_records`: two or more definitions carry it: records whose `id` or an alias equals it, plus `duplicates` of such an `id` (D7);
2. `ambiguous` `two_items`: it would match by 3-5 and another valued item holds it (a cfg twin, another file);
3. `id`: one record's `id`; 4. `alias`: one record's alias;
5. `hyphenless`: a pattern's leftmost match (`Regex::find`) spans the whole value, import's `is_hyphenless` restated (its module is private); counted only, no record (ADR-0009 holds until superseded);
6. `unmatched`.

The value as written is the `{id}` template; `{prefix}{body}`, `{body}` are 12b's config (Q10), so their candidates read `unmatched`, value shown.

### stdout

`{measurement: "constants", label, versions, wall_ms, result}`; `result` exactly these keys, all present, zeros too; counts only, never a path, name, ID or value (eval README "Output"); on overrun `result` is `"timeout"`. Illustrative:

```json
{"files":9,"skipped":1,"roots_missing":1,"items":21,"valued":13,
 "not_valued":{"broken":1,"macro":1,"byte_string":1,"escaped":1,"multi_line":1,"other":3},
 "matched":{"id":3,"alias":1},"hyphenless":1,"unmatched":3,
 "ambiguous":{"two_records":1,"two_items":4},"marked":1,"constants_ms":4}
```

`files`: listed, skipped ones in; `marked`: valued items with a bound marker; `constants_ms`: first file read to last row matched, import excluded. Invariants: `items` = `valued` + Σ `not_valued`; `valued` = Σ `matched` + `hyphenless` + `unmatched` + Σ `ambiguous`; `marked` ≤ `valued`; `skipped` ≤ `files`; `two_items` ≠ 1.

### Detail, `--out/constants/<label>/` (D3)

`constants.json`, one row per item by path bytes, `line`, start byte; `skipped.json`, `{path, cause}` by path bytes, cause `not_utf8` · `unreadable` · `unparsed` · `root_missing`.

```json
{"path":"src/rules/codes.rs","line":7,"name":"ROW_MISSING","mod_path":[],"cfg":[],
 "item_kind":"const_item","value":"RUL-012","not_valued":null,"match":"id",
 "records":["RUL-012"],"marked":[]}
```

`path` corpus-relative, `/`-separated; `line`, `name`, `mod_path`, `cfg` are `ItemRecord`'s (cfg twins differ by `cfg`); no qpath. A not-valued row: `value`, `match` null, `not_valued` its category. `match`: `id` · `alias` · `hyphenless` · `unmatched` · `two_records` · `two_items`. `records`: the sorted distinct IDs the value names (an alias: its record's `id`; `[]` for `hyphenless`, `unmatched`); `marked`: the bound `Marker.id`s in source order, on every row.

## Rules and edge cases

- WHEN two items (cfg twins, two files) hold one value that would match, both SHALL read `ambiguous two_items`; order never decides.
- WHEN an ID is defined twice (an `Import.duplicates` entry), its value SHALL read `ambiguous two_records`, never `id`.
- WHEN a file lies under two roots, it SHALL be read and counted once; WHEN `exclude` matches it, it SHALL NOT be listed.
- `result` but `constants_ms`, and the detail files, are byte-identical whatever the absolute root, cwd, creation order, LF or CRLF; nothing under the corpus is written.
- No kind (`check`), prefix, template or code shape in the reader or `constants` (`architecture.md#universal`); the Rust literal forms are language support (code README "Rules"); a string value still never yields a mention (code-refs Q2).

## Acceptance criteria

rust-developer, M: mutation -> the test turning red (eval `tests/constants_cli.rs`, code `tests/const_value.rs` unless named):

- [ ] RD-1 -- value reader (D2, D4). M: `concat!` accepted -> `macro_value_is_not_a_value`; `"\x45…"` unescaped -> `escaped_literal_is_not_a_value`; `r#` kept -> `raw_literal_value_is_its_text`; a CR kept -> `multi_line_literal_is_not_a_value`; `b"…"` accepted -> `byte_string_is_not_a_value`; `has_error` ignored -> `broken_item_is_not_a_value`; a walk below the initializer -> `deeply_nested_initializer_is_other` (10 000 parentheses).
- [ ] RD-2 -- matching (D7). M: substring or prefix -> `partial_value_never_matches`; first by path wins -> `one_value_on_two_items_is_ambiguous`; first record wins -> `one_value_naming_two_records_is_ambiguous`; `duplicates` unread -> `duplicate_definition_reads_two_records`; `is_match` or an unanchored regex -> `hyphenless_matches_the_whole_value_only` (a leftmost match shorter than the value: `unmatched`).
- [ ] RD-3 -- files (D1). M: `extensions` applied -> `extensions_never_narrow_constants`; `exclude` ignored -> `excluded_file_is_not_read`; a `target*` or hidden directory entered -> `build_and_hidden_directories_are_skipped`; counted per root -> `file_under_two_roots_is_read_once`; a missing root fatal -> `missing_root_is_counted`; empty roots measured -> `config_without_code_roots_is_exit_2`.
- [ ] RD-4 -- read-only, deterministic. M: a file written under the root -> `constants_leaves_the_corpus_untouched`; walk order -> `constants_is_byte_identical_across_roots_and_order`; a path, name, ID or value on stdout -> `constants_stdout_holds_counts_only`; a zero key dropped -> `constants_result_has_every_key`.
- [ ] RD-5 -- graph. M: `specengine-code` or `tree-sitter` in `specengine-import` -> new `build_graph.rs` `import_reads_code_as_bytes_only`; a direct `tree-sitter` in eval -> `eval_direct_dependencies_gain_exactly_the_cli_library`.
- [ ] RD-6 -- genre (D5). M: a `"check"`, `{id}`, `{prefix}`, `{body}` or fixture prefix literal in eval `constants.rs` or code `const_value.rs` -> `import_genre.rs`, its scan extended to both files, its forbidden set to both `fixtures/code-consts/*/census.toml` and those words; an outcome zero in one fixture -> new `every_constants_outcome_is_non_zero_in_both_fixtures`.
- [ ] RD-7 -- frozen bytes (08 s2, the owner's). `git diff --stat <base> -- crates/specengine-code/src/items.rs crates/specengine-code/src/hash.rs crates/specengine-code/src/levels.rs crates/specengine-code/src/ron/ Cargo.lock` prints nothing (`<base>`: the commit 12a starts from); `CODE_INDEX_FORMAT` is 4 and `crates/specengine-cli/tests/code_format_history.txt` unchanged. M (once, reverted): `CODE_INDEX_FORMAT` 5 -> `code_format_history_pins_the_code_index_dump`; one byte of recipe input in `levels.rs` -> 5a-1b's stream pin and the `hash_levels_*` goldens.

test-engineer:

- [ ] TE-1 -- `fixtures/code-consts/{one,two}` (D5): two invented conventions (prefixes, `[ids] legacy`, `hyphenless`, `exclude`, layout differ; ASCII), each a census config, records, code roots, `expected.json` with every row and count. Rows across the pair: plain, raw, escaped, byte string, `concat!`, a call, a number, an enum variant and an associated const (no item), a fn-body const (absent), `static`, an inline-`mod` const, cfg twins with one value, a cross-file duplicate value, a broken item, an alias value, a hyphenless match, a pattern matching a prefix only, a value naming two records, a duplicated ID's value, one naming nothing, a marked constant, an excluded file, a missing root, a non-UTF-8 file. Every count but `not_valued`'s and `constants_ms` non-zero in both; each `not_valued` category in at least one; the invariants hold.
- [ ] TE-2 -- both fixtures: two roots, another cwd, reverse creation, CRLF: byte-identical `result` (but ms) and detail; a mode-000 file in a temp copy: `skipped`, `unreadable`.
- [ ] TE-3 -- `git status --porcelain -- fixtures/` empty; (size, mtime, BLAKE3) of every corpus and code file kept.
- [ ] TE-4 -- pilot B, the owner's manual run (`constants_pilot.rs`, `#[ignore]`, release, `--timeout`, scratch `HOME`, foreground; `tests/pilot/mod.rs` `run_census_read_only`: corpus and code roots unchanged): invariants hold; figures equal a read-only grep tally kept outside the repository (hyphenless codes read `hyphenless`, 0 `matched`). No pilot name, path or count committed. The slice ships without it.

code-reviewer:

- [ ] CR-1 -- frozen bytes at review: RD-7's command prints nothing, `CODE_INDEX_FORMAT` 4; no manifest gains a dependency; in `specengine-code` only the new module and `lib.rs`'s `mod` / `pub use` lines change.

spec-writer:

- [x] SW-1 -- this draft (2026-10-10; refreshed 2026-10-19: S1-S16, D1-D9); 08 §2 Phase 3, §4.1; `bevy-dump.md`'s slice-12 pointers and 05 §5.5 level 2 "no slice yet" (D9); no ADR.
- [ ] SW-2 -- at shipping, room first (bytes measured 2026-10-19 at 9261491). Eval README 10 237 / 10 240: first move "Rules"' `ra` loads and RSS bullets (798 B) to `crates/specengine-ra/README.md` "Loading" (5 046 / 10 240), a pointer of ~90 B left (fallback: the `parse` row's key list, its line 619 B, to a Tier 2 canon); then the synopsis word, a `constants` row pointing to the canon, the `--config` and `tests/pilot/mod.rs` lists (~250 B). Code README 9 905 / 10 240: a module row and the test file (~170 B; 11a needs the rest). New `docs/canon/code-constants.md` (~3 KB of 12 288) and its `docs/index-canon.md` line (8 793 / 10 240, ~190 B); `CLAUDE.md` State (5 822 / 16 384); 08 §2 status; compaction; `check` clean.

## Risks

No user: the report may find nothing to bind; its worth is a cheap census. Genre: the one real shape is one project's (`&str`, hyphenless, 0 `matched` by ADR-0009); RD-6 and D5's two conventions guard the core (08 §5). Two walks: the census `[code]` (D1) is not the scheme's `code-bindings.md` "The walk", which the code index lists through; `files` may differ from `spec index`'s on one tree, not reconciled.

## Out of scope

Writing a marker or constant, from anything; enum variants, associated consts, macro-made tables, literals at emission sites; RON; templates; reach; `#[test]`; `spec`, the code index, its stamp, `spec.lock`; qpaths, package tables (D3); the scheme's `[code]` (D1); proposing `check` records; this repository's dogfooding (Q2); the hyphenless grammar; 05 §5.5 level 2 (D9).

## Open

Working answers; they frame 12b and 12c too.

| Q | Working answer | The other answer costs |
|---|---|---|
| Q1 meaning | marker-free, from an exact token: a configured node's code equals a free `const`/`static`'s whole string value (check → const); items using its name or value reach it (const → test) | markers the sole identity: a code binds only once its constant is touched; coverage invisible till then |
| Q2 this repository | not in slice 12; dogfooding is its own task if the owner wants it (WA-10) | `CHECK_CODES`, emit sites, pin test refactored; a `name` prefix, 36 records (49 with parser codes), `[code]` first |
| Q3 consumers | opt-in per project, `[code] constants` (12b); first user: the project with hyphenless codes, after the ADR superseding ADR-0009 and its migration | one project's shape in the core (`#universal`), or IDs before that ADR |
| Q4 candidates | `specengine-eval constants`, read-only; after 12b the plain reads show the facts (WA-8) | the importer placing markers (08 §4.1 literally) breaks ADR-0016, 08 §4.2.1 |
| Q5 kind | none new: `item`, `const_item`/`static_item`, `implements`; source `constant` computed, never stored (WA-4) | a lock format bump: every lock reader, verify key, golden |
| Q6 evidence | reaching tests heuristic; evidence only `@verifies` run by `spec verify --tests` | amends ADR-0044: an absence assertion counts |
| Q7 ADR H | drafted in "Later", decided with 12b | 12b breaks `architecture.md#code-identity` |
| Q8 now? | 12a only, after 9a-2, due by the hyphenless project's dry run; 12b's gates: a user ("Later"), ADR H, the lock chain 5b-1 -> 5b-2 -> 5c (3, 4a, 6a shipped); 12c after 12b, 7a | drafts under it, zero users; owner time (08 §5 "high") |
| Q9 split | 12a census (no ADR); 12b bindings (ADR H); 12c reach | one slice waits for references, outside 6a |
| Q10 template | `[code] constants = { <PREFIX> = "<template>" }`, off without `[code]`; 12a reads `{id}` | in `[ids]`: the model's scheme changes for every command |
| Q11 `#[test]` | no: each reaching item with its unit | the core reads test attributes (`docs/canon/spec-verify-tests.md` "Runnable and verifiable": "Core never looks for `#[test]`") |
| D1 files | census `[code]` `roots` minus `exclude`, `.rs` only, `harness::rust_files`' rules; a file root kept, a missing one counted; one config for the dry run | the scheme's `[code]` via `listed_code_files`: `--scheme`, `SPECENGINE_CODE_ROOTS_B`, 8a-1's `code_walk.rs` coupled; `walk::files` public: import and its README in scope |
| D2 `associated` | dropped: six `not_valued` categories | `impl` bodies walked, a second item rule beside `code-bindings.md` "Rust binding"; or `items.rs`' item set: a `CODE_INDEX_FORMAT` bump, new `ast-hash` figures inside 5b-1's freeze |
| D3 row naming | `path`, `line`, `name`, `mod_path`, `cfg`; no qpath | package tables (`layout_targets` per nearest `Cargo.toml`) and `name_files` for a field no count reads |
| D4 reader home | a new `specengine-code` module over `&Tree` and `&ItemRecord`, the node by `byte_range`; no stamp bump: it stores nothing | in eval: a direct `tree-sitter` (`build_graph.rs` red) or a hand lexer for Rust literals |
| D5 fixtures | `fixtures/code-consts/{one,two}`, two invented conventions, every outcome non-zero in both | one shape: the genre risk unguarded |
| D6 items | `bind_tree`'s analysis: items unhashed, `marked` from the same parse | `analyze_tree` and `bind_tree`: two parses, every item hashed |
| D7 duplicates | `two_records` reads `Import.duplicates` too | a duplicated ID reads `id`, a false single match |
| D8 canon | a new Tier 2 canon at shipping; README rows point to it | none fits: eval README 3 B free, `import.md` 398 B |
| D9 05 §5.5 level 2 | "no slice yet", recorded in 05 and `bevy-dump.md`; the owner's (11c or a later consumer) | 10b's parking silently drops level 2 |

Q1-Q11 (2026-10-10; Q2, Q8, Q11 re-pointed 2026-10-19) and D1-D9 (2026-10-19, the refresh against shipped 6a, 6b-1, 8a-1, 10a) decided under the owner's pre-approval, review open. S1-S16: the refresh's stale points, fixed where cited. SW refinement: empty `[code] roots` exit 2, cause `unparsed`.

## Working answers

[P] proven at the cited place, [A] assumed. WA-2 Value [P/A], WA-3 Match [A]: "Data".

- **WA-1 [A]** "Machine": deterministic, derived, no human marker (08 AC-8, 05 §5.5).
- **WA-4 [P/A]** The lock key fits format 1 (`spec-lock.md` "Entry"); marker and constant of one key, one row; constant bindings `unversioned`, locked at R. Re-read against 5b-1's format 2 when 12b is drafted.
- **WA-5 [A]** Reach (12c): naming the constant (heuristic: no name resolution) or holding its value; confidence and unit shown; never a verify row, locked or run.
- **WA-6 [P]** Nothing writes code (ADR-0016); markers only in new or touched code (`architecture-code.md#markers`).
- **WA-7 [P]** The report lives in `specengine-eval`; import's stdout keys stay exact.
- **WA-8 [A]** After 12b, bound constants show in `refs`, `symbols --node` and their MCP twins (`docs/canon/mcp-code.md`), count as mapped in `unmapped` and Health rows 4-5 (ADR-0051).
- **WA-9 [A]** `[code] constants` closed: a prefix not in `[ids]`, an unknown placeholder, an empty rendering -> `specengine.toml:<line>:`, exit 2; the template in 6a's `code_fp` beside `frameworks`.
- **WA-10 [P]** Here: `CHECK_CODES` 36 literals emitted as literals, parser codes an enum (13), `[ids]` only `ADR`, no `[code]`; codes are canon rows (`spec-check-process.md`), not nodes.
- **WA-11 [P]** Consumers (read-only greps, 2026-10-10): one holds free `&str` constants with hyphenless codes, asserted mostly as literals in tests; the other none ID-shaped.
- **WA-12 [P]** A fresh constant binding is at best level 3; level 1 needs `@verifies` run.
- **WA-13 [P/A]** A constant's body is its value: until 5b a rename reads `code_ahead`; 5b gives `[body]`; under 124 B 5c never follows.
- **WA-14 [P]** `spec check`, `CHECK_CODES` (36) unchanged.
- **WA-15 [A]** Deferring costs nothing: the lock key fits, the index is rebuildable.
- **A2 [A]** Pilot B's census `[code]` is the right code source for its dry run. **A3 [P]** `Import.records` holds first definitions only; the rest are `Import.duplicates`.

## Dependencies and ordering

12a needs slice 1 `code-refs` (shipped: `bind_tree`, `markers_in`) and the importer, not 3, 4a, 5b or 6a. Order (08 s2, 2026-10-16): 9a-1 `health-code` -> 9a-2 `health-code-ui` -> 12a; neither touches eval or code [A]. Until 5b-1 ships, `levels.rs`, `hash.rs`, `ron/` and `Cargo.lock`'s tree-sitter entries are frozen (08 s2); 12a also keeps `items.rs` and all of `Cargo.lock` (RD-7, CR-1). Size M, low end, no split: the reader has no other consumer. TE-4 is the owner's run.

## Later: 12b and 12c

Not drafted. **Triggers**, either: the owner opens this repository's dogfooding task (Q2); or the consumer project with hyphenless codes migrates (second in 08 §2 Phase 1's order), after the ADR superseding ADR-0009 and its code-tables decision. 12b also waits for ADR H and the lock chain 5b-1 -> 5b-2 -> 5c (3, 4a, 6a shipped); 12c for 12b and 7a (`delta.tests_reaching`).

**ADR H** -- not decided: drafted with 12b once a user exists, numbered then. "A configured string constant binds a node without a marker": per `[ids]` prefix in `[code] constants`, a free `const`/`static` whose whole value, one plain string literal, equals a node's rendered code binds it as `implements`, unversioned; exact or nothing (one value on two items or naming two nodes: ambiguous); locked and judged as a marker binding of the same key, a marker on it adding revision and levels; reaching items heuristic, never evidence or run; nothing writes code. Why: a check code is one exact token in spec, code and tests. Cost: a second identity source (amends ADR-0020); a value edit re-points; enum, associated, macro codes stay unbound; `lock accept` the only review record. Canon: `architecture.md#markers`; "the marker is a link's identity" amended in both `#code-identity`; a bullet in `architecture-code.md#markers`; the how-to in a Tier 2 canon beside 12a's (`code-bindings.md` 10 842 / 12 288).

**12b `machine-bindings`**: `[code] constants` (WA-9); constant bindings in `refs`, `symbols --node`, `unmapped`, their MCP twins, Health rows 4-5, `verify`, `lock accept`; a code-index value column: `CODE_INDEX_FORMAT` (4) and `INDEX_FORMAT` (10) bumps; verify's `broken_binding value_changed`; `spec-lock.md`, `spec-verify.md`, `symbol-index.md` amended; a fixture chain under two templates. Tests: `unmarked_constant_binds_its_node`, `constant_binds_only_an_exact_value`, `one_value_on_two_items_cannot_verify`, `marker_and_constant_of_one_key_are_one_row`, `changed_constant_value_breaks_the_binding`, `constant_binding_writes_no_code`; `string_literal_ids_are_not_mentions` stays green.

**12c `constant-reach`**: reach rows (WA-5); exact references only via layer C (11a), advisory. Tests: `reach_is_never_run`, `reach_is_never_a_verify_row`; fixture tests asserting the literal, the name, an absence: listed, none `verified`.

## Implementation

Not started. To record: modules, TE-4's figures (outside the repository), deviations with reasons.
