---
class: spec
status: draft
scope: [crates/specengine-eval, crates/specengine-code, fixtures]
ref: Phase 3 plan 2026-10-10, slice 12a of 12 machine-bindings (split 12a/12b/12c; Q1-Q11 decided under the owner's pre-approval, review open; 12b, 12c not drafted); 05 s5.1, s5.5; 08 s2 Phase 3, s4.1
adrs: []
---

# constant-census: which string constants carry a record's code (slice 12a)

## Why

An agent changing a check needs the record defining the code, the constant carrying it, where it is emitted and the tests asserting it. Today it greps, and coverage stays invisible: which codes have no constant, no test, no record. 08 §2 Phase 3 wants this `check → const → test` chain derived, not hand-marked; 08 §4.1 has the importer carry check codes over.

Slice 12 is split (Q9). **12a** (this): a read-only `specengine-eval` report of which free string constants of a project's code carry a code of its records; no ADR (ADR-0008, -0016, -0020 applied: it binds, locks and writes nothing). **12b** `machine-bindings` and **12c** `constant-reach` are not drafted: no user exists yet (WA-10, WA-11; "Later").

## Description and interactions

- The owner or an agent preparing a consumer project's migration runs `specengine-eval constants` beside `census` and `import` (08 §4.2.1 dry run).
- `specengine-code` gains the value reader, a pure function over an analysed item; `specengine-eval` the subcommand, with records from `specengine-import` and the parser and slice 1's binder from `specengine-code`, both existing dependencies. `specengine-import` stays tree-sitter-free (`docs/canon/import.md` "Links and code scan").
- Unchanged: import's outputs, every `spec` command, core, store, MCP, HTTP, UI, plugin, hook, CI. Nothing writes code, a marker, the corpus or the queue (WA-6).

## Data

### Invocation and inputs

```
specengine-eval constants [--pilot <dir>] --out <scratch-dir> [--label <name>] [--config <toml>]
```

Flags, labels, exits as `census` (eval README "CLI contract"); fixture `fixtures/code-consts`; `--config` the census config.

- **Files**: `.rs` regular files under the config's `[code] roots` minus `exclude`, walked as the import's code scan (dot-directories, names starting `target` skipped; a missing root counted); `extensions`, `strip` unused; `.ron` never. Not UTF-8 or unreadable: `skipped`.
- **Items**: each `const_item`, `static_item` `analyze_tree` collects (`code-bindings.md` "Rust binding"; none in function bodies), with its `qpath`.
- **Records**: `import::run` over the same config: each `ImportRecord`'s `id` and `aliases` (`[ids] legacy`); the `[ids] hyphenless` regexes.

### Value (WA-2)

The text between the quotes of an item's whole initializer when that is one plain `"…"` or raw `r#*"…"#*` string literal, never unescaped. Else `not_valued`, the first category that applies: `broken` (`has_error()`); `associated` (in an `impl` or `trait`); `macro` (`concat!`, `include_str!`, any call); `byte_string` (`b"…"`, `br"…"`, `c"…"`, `cr"…"`, a char); `escaped` (a plain literal holding `\`); `multi_line` (a literal holding LF or CR); `other` (no initializer, a number, a path, `&[..]`, parentheses, a cast).

### Match (WA-3)

Whole value, byte for byte (no prefix, substring, case fold or trim); each valued item takes the first that applies:

1. `ambiguous` `two_records`: it equals the `id` or an alias of two or more records (a duplicate definition too);
2. `ambiguous` `two_items`: it would match by 3-5 and another valued item holds it (a cfg twin, another file);
3. `id`: one record's `id`; 4. `alias`: one record's alias;
5. `hyphenless`: a `hyphenless` regex matches the whole value; counted only, no record (ADR-0009 holds until superseded);
6. `unmatched`.

The value as written is the `{id}` template; `{prefix}{body}`, `{body}` are 12b's config (Q10), so their candidates read `unmatched`, value shown. `marked`: slice 1's binder (`bind.rs`) binds a marker to the item, any relation, token as written (no ID resolved: no index).

### stdout

`{measurement: "constants", label, versions, wall_ms, result}`; `result` exactly these keys, all present, zeros too; counts only, never a path, ID or value (eval README "Output"). Illustrative:

```json
{"files":9,"skipped":0,"roots_missing":0,"items":21,"valued":12,
 "not_valued":{"broken":1,"associated":1,"macro":1,"byte_string":1,"escaped":1,"multi_line":1,"other":3},
 "matched":{"id":3,"alias":1},"hyphenless":1,"unmatched":3,
 "ambiguous":{"two_records":1,"two_items":3},"marked":1,"constants_ms":4}
```

Invariants: `items` = `valued` + Σ `not_valued`; `valued` = Σ `matched` + `hyphenless` + `unmatched` + Σ `ambiguous`; `marked` ≤ `valued`.

### Detail, `--out/constants/<label>/`

`constants.json`, one row per item by path bytes, line, qpath; `skipped.json`, `{path, cause}`, cause `not_utf8` · `unreadable` · `root_missing`.

```json
{"path":"crates/rules/src/codes.rs","line":7,"qpath":"crates/rules::codes::ROW_MISSING",
 "item_kind":"const_item","value":"CHK-012","not_valued":null,"match":"id",
 "records":["CHK-012"],"marked":[]}
```

A not-valued row: `value`, `match` null, `not_valued` its category. `records`: the sorted IDs the value names (`[]` for `hyphenless`, `unmatched`); `marked`: the bound marker tokens by line.

## Rules and edge cases

- WHEN two items (cfg twins, two files) hold one value that would match, both SHALL read `ambiguous two_items`; order never decides.
- `result` but `constants_ms`, and the detail files, are byte-identical whatever the absolute root, cwd, creation order, LF or CRLF; nothing under the corpus is written.
- No kind (`check`), prefix, template or code shape in the reader or `constants` (`architecture.md#universal`); a string value still never yields a mention (code-refs Q2).

## Acceptance criteria

rust-developer, M: mutation -> the test turning red (eval `tests/constants_cli.rs`, code `tests/const_value.rs` unless named):

- [ ] RD-1 -- value reader. M: an associated const accepted -> `associated_const_has_no_value`; `concat!` accepted -> `macro_value_is_not_a_value`; `"\x45…"` unescaped -> `escaped_literal_is_not_a_value`; `r#` kept -> `raw_literal_value_is_its_text`; a CR kept -> `multi_line_literal_is_not_a_value`.
- [ ] RD-2 -- matching. M: substring or prefix -> `partial_value_never_matches`; first by path wins -> `one_value_on_two_items_is_ambiguous`; first record wins -> `one_value_naming_two_records_is_ambiguous`; unanchored regex -> `hyphenless_matches_the_whole_value_only`.
- [ ] RD-3 -- read-only, deterministic. M: a file written under the root -> `constants_leaves_the_corpus_untouched`; walk order -> `constants_is_byte_identical_across_roots_and_order`; a path or value on stdout -> `constants_stdout_holds_counts_only`.
- [ ] RD-4 -- graph. M: `specengine-code` or `tree-sitter` in `specengine-import` -> new `build_graph.rs` `import_reads_code_as_bytes_only`.
- [ ] RD-5 -- genre. M: a `"check"`, prefix or template literal in `constants` or the reader -> `import_genre.rs`, extended to both and to `fixtures/code-consts/census.toml`.

test-engineer:

- [ ] TE-1 -- `fixtures/code-consts/` (census config, records, code roots) + `expected.json`, a row each: plain, raw, escaped, byte string, `concat!`, associated, variant (no item), fn-body const (absent), `static`, cfg twins with one value, a cross-file duplicate, a broken item, an alias value, a hyphenless match, a value naming two records, one naming nothing, a marked constant; every `result` key non-zero.
- [ ] TE-2 -- two roots, another cwd, reverse creation, CRLF: byte-identical `result` (but ms) and detail.
- [ ] TE-3 -- `git status --porcelain -- fixtures/` empty; (size, mtime, BLAKE3) of every corpus and code file kept.
- [ ] TE-4 -- pilots, manual (`#[ignore]`, release, `--timeout`, scratch `HOME`, foreground): invariants hold; figures equal a read-only grep tally kept outside the repository (hyphenless codes read `hyphenless`, 0 `matched`); TE-3's proof. No pilot name, path or count committed.

spec-writer:

- [x] SW-1 -- this draft; 08 §2 Phase 3 (slice list, machine-bindings line) and §4.1 (check codes: "constant bindings; candidates reported", ADR-0016); no ADR.
- [ ] SW-2 -- at shipping: the `constants` row in the eval README, the reader in the code README (both under 60 B of their cap: detail moves to Tier 2 first); `CLAUDE.md` State; compaction; `check` clean.

## Risks

No user: the report may find nothing to bind; its worth is a cheap census. Genre: the one real shape is one project's (`&str`, hyphenless); RD-5 and a two-shape fixture guard the core (08 §5). Two walks: the import's code walk is not `code-bindings.md` "The walk"; `files` may differ from `spec refs`', not reconciled.

## Out of scope

Writing a marker or constant, from anything; enum variants, associated consts, macro-made tables, literals at emission sites; RON; templates; reach; `#[test]`; `spec`, the index, `spec.lock`; proposing `check` records; this repository's dogfooding (Q2); the hyphenless grammar; the Bevy dump's level 2 assertions ("Open").

## Open

Working answers, owner pre-approved, review open; they frame 12b and 12c too.

| Q | Working answer | The other answer costs |
|---|---|---|
| Q1 meaning | marker-free, from an exact token: a configured node's code equals a free `const`/`static`'s whole string value (check → const); items using its name or value reach it (const → test) | markers the sole identity: a code binds only once its constant is touched; coverage invisible till then |
| Q2 this repository | not in slice 12; dogfooding is its own task if the owner wants it (WA-10) | `CHECK_CODES`, emit sites, pin test refactored; a `name` prefix, 35 records (48 with parser codes), `[code]` first |
| Q3 consumers | opt-in per project, `[code] constants` (12b); first user: the project with hyphenless codes, after the ADR superseding ADR-0009 and its migration | one project's shape in the core (`#universal`), or IDs before that ADR |
| Q4 candidates | `specengine-eval constants`, read-only; after 12b the plain reads show the facts (WA-8) | the importer placing markers (08 §4.1 literally) breaks ADR-0016, 08 §4.2.1 |
| Q5 kind | none new: `item`, `const_item`/`static_item`, `implements`; source `constant` computed, never stored (WA-4) | a lock format bump: every lock reader, verify key, golden |
| Q6 evidence | reaching tests heuristic; evidence only `@verifies` run by `spec verify --tests` | amends ADR-0044: an absence assertion counts |
| Q7 ADR H | drafted in "Later", decided with 12b | 12b breaks `architecture.md#code-identity` |
| Q8 now? | not 12b, 12c: 12a is unblocked, due by the hyphenless project's dry run; 12b on a trigger once 3, 4a, 6a ship; 12c after 12b, 7a | drafts under it, zero users; owner time (08 §5 "high") |
| Q9 split | 12a census (no ADR); 12b bindings (ADR H); 12c reach | one slice waits for references, outside 6a |
| Q10 template | `[code] constants = { <PREFIX> = "<template>" }`, off without `[code]`; 12a reads `{id}` | in `[ids]`: the model's scheme changes for every command |
| Q11 `#[test]` | no: each reaching item with its unit | the core reads test attributes (`verify-tests.md` WA-4 avoids it) |

**Contradiction, owner's call**: `bevy-dump.md` ("Reader", "Out of scope", "Dependencies") hands the dump's `dependency`, `system_sets`, `hierarchy`, conditions, `conflicts`, `#[require(..)]` (05 §5.5 level 2) to slice 12; slice 12 as split is `check → const → test` only, so they belong to no slice yet.

## Working answers

[P] proven at the cited place, [A] assumed. WA-2 Value [P/A], WA-3 Match [A]: "Data".

- **WA-1 [A]** "Machine": deterministic, derived, no human marker (08 AC-8, 05 §5.5).
- **WA-4 [P/A]** The lock key fits (`spec-lock.md` "Entry"); marker and constant of one key, one row; constant bindings `unversioned`, locked at R.
- **WA-5 [A]** Reach (12c): naming the constant (heuristic: no name resolution) or holding its value; confidence and unit shown; never a verify row, locked or run.
- **WA-6 [P]** Nothing writes code (ADR-0016); markers only in new or touched code (`architecture-code.md#markers`).
- **WA-7 [P]** The report lives in `specengine-eval`; import's stdout keys stay exact.
- **WA-8 [A]** After 12b, bound constants show in `refs`, `symbols --node`, count as mapped in `unmapped`.
- **WA-9 [A]** `[code] constants` closed: a prefix not in `[ids]`, an unknown placeholder, an empty rendering -> `specengine.toml:<line>:`, exit 2; the template in 6a's `code_fp`.
- **WA-10 [P]** Here: `CHECK_CODES` 35 literals emitted as literals, parser codes an enum (13), `[ids]` only `ADR`, no `[code]`; codes are canon rows (`spec-check-process.md`), not nodes.
- **WA-11 [P]** Consumers (read-only greps, 2026-10-10): one holds free `&str` constants with hyphenless codes, asserted mostly as literals in tests; the other none ID-shaped.
- **WA-12 [P]** A fresh constant binding is at best level 3; level 1 needs `@verifies` run.
- **WA-13 [P/A]** A constant's body is its value: until 5b a rename reads `code_ahead`; 5b gives `[body]`; under 124 B 5c never follows.
- **WA-14 [P]** `spec check`, `CHECK_CODES` (36 after 4b-2) unchanged.
- **WA-15 [A]** Deferring costs nothing: the lock key fits, the index is rebuildable.

## Dependencies and ordering

12a needs slice 1 `code-refs` (shipped 2026-10-10, the binder for `marked`), not 3, 4a or 6a; run it in the hyphenless project's migration dry run. 12b, 12c: below.

## Later: 12b and 12c

Not drafted. **Triggers**, either: the owner opens this repository's dogfooding task (Q2); or the consumer project with hyphenless codes migrates (second in 08 §2 Phase 1's order), after the ADR superseding ADR-0009 and its code-tables decision. 12b also waits for 3 `spec-lock`, 4a, 6a to ship; 12c for 12b and 7a (`delta.tests_reaching`).

**ADR H** -- not decided: drafted with 12b once a user exists, numbered then. "A configured string constant binds a node without a marker": per `[ids]` prefix in `[code] constants`, a free `const`/`static` whose whole value, one plain string literal, equals a node's rendered code binds it as `implements`, unversioned; exact or nothing (one value on two items or naming two nodes: ambiguous); locked and judged as a marker binding of the same key, a marker on it adding revision and levels; reaching items heuristic, never evidence or run; nothing writes code. Why: a check code is one exact token in spec, code and tests. Cost: a second identity source (amends ADR-0020); a value edit re-points; enum, associated, macro codes stay unbound; `lock accept` the only review record. Canon: `architecture.md#markers`; "the marker is a link's identity" amended in both `#code-identity`; a bullet in `architecture-code.md#markers`; the how-to in a new Tier 2 canon (`code-bindings.md` is 12 B under its cap).

**12b `machine-bindings`**: `[code] constants` (WA-9); constant bindings in `refs`, `symbols --node`, `unmapped`, `verify`, `lock accept`; a store value column, `INDEX_FORMAT` bump; verify's `broken_binding value_changed`; `spec-lock.md`, `spec-verify.md`, `symbol-index.md` amended; a fixture chain under two templates. Tests: `unmarked_constant_binds_its_node`, `constant_binds_only_an_exact_value`, `one_value_on_two_items_cannot_verify`, `marker_and_constant_of_one_key_are_one_row`, `changed_constant_value_breaks_the_binding`, `constant_binding_writes_no_code`; `string_literal_ids_are_not_mentions` stays green.

**12c `constant-reach`**: reach rows (WA-5); exact references only via layer C (11a), advisory. Tests: `reach_is_never_run`, `reach_is_never_a_verify_row`; fixture tests asserting the literal, the name, an absence: listed, none `verified`.

## Implementation

Not started. To record: modules, TE-4's figures (outside the repository), deviations with reasons.
