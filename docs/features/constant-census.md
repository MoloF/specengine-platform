---
class: spec
status: shipped
scope: [crates/specengine-eval, crates/specengine-code, fixtures]
ref: Phase 3 plan 2026-10-10, slice 12a of 12 machine-bindings (split 12a/12b/12c); refreshed 2026-10-19 against shipped 6a, 6b-1, 8a-1, 10a (S1-S16, D1-D9); 05 s5.1, s5.5; 08 s2 Phase 3, s4.1; Rust 1 iteration, review accepted, tests 1 round; TE-4 the owner's
shipped: 2026-10-19
adrs: []
---

# constant-census (12a): which string constants carry a record's code

## Why

An agent changing a check needs the record defining the code, the constant carrying it, where it is emitted and the tests asserting it. Today it greps, and coverage stays invisible: which codes have no constant, no test, no record. 08 §2 Phase 3 wants this `check → const → test` chain derived, not hand-marked; 08 §4.1 has the importer carry check codes over.

Slice 12 is split (Q9). **12a** (this): a read-only `specengine-eval` report of which free string constants of a project's code carry a code of its records; no ADR (ADR-0008, -0016, -0020 applied: it binds, locks and writes nothing). **12b** `machine-bindings` and **12c** `constant-reach` are not drafted: no user exists yet ("Later").

## Description and interactions

Canon: `docs/canon/code-constants.md` (reader, value, files, records, match, output, refusal and limits); the eval README `constants` row, the code README `const_value` row.

- `specengine-eval constants` runs beside `census` and `import` (08 §4.2.1 dry run): records through `import::run`, files by `harness::rust_files`' rules (D1), items and markers through `specengine-code` (`RustParser`, `bind_tree`, `markers_in`, D6). No dependency added; `specengine-import` keeps neither `tree-sitter` nor `specengine-code` (RD-5).
- Unchanged: import's outputs and stdout keys, every `spec` command, the code index and its stamp, core, store, CLI, MCP, HTTP, UI, plugin, hook, CI.

## Data

`docs/canon/code-constants.md`: "Reader" (D4), "Value" (D2), "Files" (D1), "Records" (D7), "Match", "Output" (D3). Fixtures `fixtures/code-consts/{one,two}` (D5), each with `expected.json`.

## Rules and edge cases

`docs/canon/code-constants.md` "Match" (order never decides; a duplicated ID reads `two_records`), "Files" (a file under two roots read once; `exclude` honoured), "Output" (byte-identical across root, cwd, creation order, LF or CRLF; the corpus never written), "Refusal and known limits". Genre: `architecture.md#universal`, enforced by RD-6.

## Acceptance criteria

rust-developer, M: mutation -> the test turning red (eval `tests/constants_cli.rs`, code `tests/const_value.rs` unless named):

- [x] RD-1 -- value reader (D2, D4). M: `concat!` accepted -> `macro_value_is_not_a_value`; `"\x45…"` unescaped -> `escaped_literal_is_not_a_value`; `r#` kept -> `raw_literal_value_is_its_text`; a CR kept -> `multi_line_literal_is_not_a_value`; `b"…"` accepted -> `byte_string_is_not_a_value`; `has_error` ignored -> `broken_item_is_not_a_value`; a walk below the initializer -> `deeply_nested_initializer_is_other` (10 000 parentheses).
- [x] RD-2 -- matching (D7, definitions only). M: substring or prefix -> `partial_value_never_matches`; first by path wins -> `one_value_on_two_items_is_ambiguous`; first record wins -> `one_value_naming_two_records_is_ambiguous`; a `Role::Reference` counted as a definition -> `a_reference_row_is_no_second_definition`; `is_match` or an unanchored regex -> `hyphenless_matches_the_whole_value_only` (a leftmost match shorter than the value: `unmatched`). `duplicates` unread is an equivalent mutant (`Import.records` holds every definition); `duplicate_definition_reads_two_records` pins the outcome.
- [x] RD-3 -- files (D1). M: `extensions` applied -> `extensions_never_narrow_constants`; `exclude` ignored -> `excluded_file_is_not_read`; a `target`, `target.*` or hidden directory entered -> `build_and_hidden_directories_are_skipped`; `targets` skipped -> `a_targets_directory_is_entered`; counted per root -> `file_under_two_roots_is_read_once`; a missing root fatal -> `missing_root_is_counted`; empty roots measured -> `config_without_code_roots_is_exit_2`.
- [x] RD-4 -- read-only, deterministic. M: a file written under the root -> `constants_leaves_the_corpus_untouched`; walk order -> `constants_is_byte_identical_across_roots_and_order`; a path, name, ID or value on stdout -> `constants_stdout_holds_counts_only`; a zero key dropped -> `constants_result_has_every_key`.
- [x] RD-5 -- graph. M: `specengine-code` or `tree-sitter` in `specengine-import` -> new `build_graph.rs` `import_reads_code_as_bytes_only`; a direct `tree-sitter` in eval -> `eval_direct_dependencies_gain_exactly_the_cli_library`.
- [x] RD-6 -- genre (D5). M: a `"check"`, `{id}`, `{prefix}`, `{body}` or fixture prefix literal in eval `constants.rs` or code `const_value.rs` -> `import_genre.rs`, its scan extended to both files, its forbidden set to both `fixtures/code-consts/*/census.toml` and those words; an outcome zero in one fixture -> new `every_constants_outcome_is_non_zero_in_both_fixtures`.
- [x] RD-7 -- frozen bytes (08 s2, the owner's). `git diff --stat <base> -- crates/specengine-code/src/items.rs crates/specengine-code/src/hash.rs crates/specengine-code/src/levels.rs crates/specengine-code/src/ron/ Cargo.lock` prints nothing (`<base>`: 90eb982); `CODE_INDEX_FORMAT` is 4 and `crates/specengine-cli/tests/code_format_history.txt` unchanged. M (once, reverted): `CODE_INDEX_FORMAT` 5 -> `code_format_history_pins_the_code_index_dump`; one byte of recipe input in `levels.rs` -> 5a-1b's stream pin and the `hash_levels_*` goldens.

test-engineer:

- [x] TE-1 -- `fixtures/code-consts/{one,two}` (D5): two invented conventions (prefixes, `[ids] legacy`, `hyphenless`, `exclude`, layout differ; ASCII), each a census config, records, code roots, `expected.json` with every row and count. Rows across the pair: plain, raw, escaped, byte string, `concat!`, a call, a number, an enum variant and an associated const (no item), a fn-body const (absent), `static`, an inline-`mod` const, cfg twins with one value, a cross-file duplicate value, a broken item, an alias value, a hyphenless match, a pattern matching a prefix only, a value naming two records, a duplicated ID's value, one naming nothing, a marked constant, an excluded file, a missing root, a non-UTF-8 file. Every count but `not_valued`'s and `constants_ms` non-zero in both; each `not_valued` category in at least one; the invariants hold.
- [x] TE-2 -- both fixtures: two roots, another cwd, reverse creation, CRLF: byte-identical `result` (but ms) and detail; a mode-000 file in a temp copy: `skipped`, `unreadable`.
- [x] TE-3 -- `git status --porcelain -- fixtures/` empty; (size, mtime, bytes) of every corpus and code file kept (bytes, not BLAKE3: eval has no `blake3`, F5).
- [ ] TE-4 -- pilot B, the owner's manual run (`constants_pilot.rs`, `#[ignore]`, release, `--timeout`, scratch `HOME`, foreground; `tests/pilot/mod.rs` `run_census_read_only`: corpus and code roots unchanged): invariants hold; figures equal a read-only grep tally kept outside the repository (`SPECENGINE_CONSTANTS_TALLY_B`; hyphenless codes read `hyphenless`, 0 `matched`). No pilot name, path or count committed. The slice ships without it.

code-reviewer:

- [x] CR-1 -- frozen bytes at review: RD-7's command prints nothing, `CODE_INDEX_FORMAT` 4; no manifest gains a dependency; in `specengine-code` only the new module and `lib.rs`'s `mod` / `pub use` lines change.

spec-writer:

- [x] SW-1 -- this draft (2026-10-10; refreshed 2026-10-19: S1-S16, D1-D9); 08 §2 Phase 3, §4.1; `bevy-dump.md`'s slice-12 pointers and 05 §5.5 level 2 "no slice yet" (D9); no ADR.
- [x] SW-2 -- at shipping: the eval README's `ra` loads and RSS bullets moved to `crates/specengine-ra/README.md` "Loading", a pointer left; then the `constants` synopsis word, row, `--config` and refusal, the `tests/pilot/mod.rs` list and `SPECENGINE_CONSTANTS_TALLY_B`; the code README `const_value` row and test; new `docs/canon/code-constants.md` and its `docs/index-canon.md` line; `CLAUDE.md` State; 08 §2; this compaction; `check` clean.

## Out of scope

Writing a marker or constant, from anything; enum variants, associated consts, macro-made tables, literals at emission sites; RON; templates; reach; `#[test]`; `spec`, the code index, its stamp, `spec.lock`; qpaths, package tables (D3); the scheme's `[code]` (D1); proposing `check` records; this repository's dogfooding (Q2); the hyphenless grammar; 05 §5.5 level 2 (D9).

## Open

Decided under the owner's pre-approval (Q1-Q11 2026-10-10, D1-D9 2026-10-19; F1, F2 at review):

- **D1** files: the census `[code]` `roots` minus `exclude`, `.rs` only, `harness::rust_files`' rules (F2: `target`, `target.*`), not the scheme's walk. **D2** six `not_valued` categories, no `associated`. **D3** rows by `path`, `line`, `name`, `mod_path`, `cfg`; no qpath. **D4** the reader in `specengine-code`, no stamp bump. **D5** two invented fixture conventions, every outcome non-zero in both. **D6** `bind_tree`'s items, one parse, `marked` from it. **D7** `two_records` reads `Import.duplicates` too; F1: definitions only. **D8** a Tier 2 canon at shipping. **D9** 05 §5.5 level 2: "no slice yet", the owner's.
- **Q1** meaning: marker-free, from an exact token: a configured node's code equals a free `const`/`static`'s whole string value (check → const); items using its name or value reach it (const → test). **Q2** this repository: not in slice 12; dogfooding is its own task. **Q3** consumers: opt-in per project, `[code] constants` (12b); first user the project with hyphenless codes, after the ADR superseding ADR-0009 and its migration. **Q4** candidates: `specengine-eval constants`, read-only; never the importer placing markers (ADR-0016). **Q5** kind: none new (`item`, `implements`); source `constant` computed, never stored. **Q6** evidence: reach heuristic; evidence only `@verifies` run by `spec verify --tests`. **Q7** ADR H decided with 12b. **Q8** 12a only now. **Q9** split 12a / 12b / 12c. **Q10** template `[code] constants = { <PREFIX> = "<template>" }`, off without `[code]`; 12a reads `{id}` only. **Q11** no `#[test]` reading: each reaching item with its unit.

## Later: 12b and 12c

Not drafted. **Triggers**, either: the owner opens this repository's dogfooding task (Q2); or the consumer project with hyphenless codes migrates (second in 08 §2 Phase 1's order), after the ADR superseding ADR-0009 and its code-tables decision. 12b also waits for ADR H and the lock chain 5b-1 -> 5b-2 -> 5c (3, 4a, 6a shipped); 12c for 12b and 7a (`delta.tests_reaching`).

**Working answers carried** ([A] assumed, [P] proven): the lock key fits format 1, marker and constant of one key one row, constant bindings `unversioned`, locked at R; re-read against 5b-1's format 2 [P/A]. Reach (12c): naming the constant (no name resolution) or holding its value; confidence and unit shown; never a verify row, locked or run [A]. After 12b, bound constants show in `refs`, `symbols --node`, their MCP twins, `unmapped` and Health rows 4-5 (ADR-0051) [A]. `[code] constants` closed: a prefix not in `[ids]`, an unknown placeholder, an empty rendering -> `specengine.toml:<line>:`, exit 2; the template in 6a's `code_fp` beside `frameworks` [A]. A fresh constant binding is at best level 3 [P]. A constant's body is its value: until 5b a rename reads `code_ahead`; under 124 B 5c never follows [P/A]. Here: `CHECK_CODES` 36 literals, parser codes an enum (13), `[ids]` only `ADR`, no `[code]` [P].

**ADR H** -- not decided: drafted with 12b once a user exists, numbered then. "A configured string constant binds a node without a marker": per `[ids]` prefix in `[code] constants`, a free `const`/`static` whose whole value, one plain string literal, equals a node's rendered code binds it as `implements`, unversioned; exact or nothing (one value on two items or naming two nodes: ambiguous); locked and judged as a marker binding of the same key, a marker on it adding revision and levels; reaching items heuristic, never evidence or run; nothing writes code. Why: a check code is one exact token in spec, code and tests. Cost: a second identity source (amends ADR-0020); a value edit re-points; enum, associated, macro codes stay unbound; `lock accept` the only review record. Canon: `architecture.md#markers`; "the marker is a link's identity" amended in both `#code-identity`; a bullet in `architecture-code.md#markers`; the how-to in `docs/canon/code-constants.md`.

**12b `machine-bindings`**: `[code] constants`; constant bindings in `refs`, `symbols --node`, `unmapped`, their MCP twins, Health rows 4-5, `verify`, `lock accept`; a code-index value column: `CODE_INDEX_FORMAT` (4) and `INDEX_FORMAT` (10) bumps; verify's `broken_binding value_changed`; `spec-lock.md`, `spec-verify.md`, `symbol-index.md` amended; a fixture chain under two templates. Tests: `unmarked_constant_binds_its_node`, `constant_binds_only_an_exact_value`, `one_value_on_two_items_cannot_verify`, `marker_and_constant_of_one_key_are_one_row`, `changed_constant_value_breaks_the_binding`, `constant_binding_writes_no_code`; `string_literal_ids_are_not_mentions` stays green.

**12c `constant-reach`**: reach rows; exact references only via layer C (11a), advisory. Tests: `reach_is_never_run`, `reach_is_never_a_verify_row`; fixture tests asserting the literal, the name, an absence: listed, none `verified`.

## Implementation

| Module | What it does |
|---|---|
| code `const_value.rs` (new) | `const_value`, `ConstValue`, `NotValued` (`ALL`, `as_str`): canon "Reader", "Value" |
| code `lib.rs` | `pub mod const_value;` and its `pub use` only (CR-1) |
| eval `constants.rs` (new) | `prepare` (empty roots refused before `--out`), `run`, `list_files` ("Files"), `Records::{new, classify}` and `match_rows` ("Records", "Match"), `ConstantsResult`, `constants.json`, `skipped.json` |
| eval `main.rs` | `Constants(CommonArgs)` and its `measure_with` arm; the `--config` doc names `constants` |
| eval `harness.rs` | `PILOT_CENSUS_CONFIG`'s doc names `import` and `constants` |
| tests, fixtures | code `const_value.rs`; eval `constants_cli.rs`, `constants_pilot.rs`, `build_graph.rs`, `import_genre.rs`, `label_cli.rs`; `fixtures/code-consts/{one,two}` |

Deviations, accepted in review (F1, F2 owner pre-approved), all in the canon:

1. F1: definitions only (`Role::Definition`); A3 ("`Import.records` holds first definitions only") was false: it holds references and every definition, so the `duplicates` join adds nothing. A value naming an ID only referenced reads `unmatched`, `records: []`.
2. F2: `target` and `target.*` skipped, `targets` entered (the harness rule), not "names starting `target`".
3. `constants_ms` starts at the root listing (grammar load included), not the first file read.
4. The reader's `None` also covers an item of another tree.
5. Roots: a missing root configured twice counts once; a walk error drops the whole root as `roots_missing`; a non-`.rs` file root is silent (both "Known limits").
6. `marked` keeps an ID-less marker as `""`.
7. `name` is `Option`: `null` if ever absent.
8. `unparsed` is reachable only by cancellation (F3).

Also: stdout keys are alphabetical (serde_json map), unlike the draft's example; TE-3 compares bytes (F5). Review nits left to the developer, optional: the ambiguous rustdoc link in `constants.rs`'s header (`cargo doc` is not in CI); a stderr note for a non-`.rs` file root; re-pointing the code's D-number comments to the canon. TE-4 stays the owner's. Tests: round 1 green: `const_value` 14/14, `constants_cli` 24/24, `import_genre` 15/15, `build_graph` 38/38; 8/8 mutations killed, RD-7's two included; F1, F2 pinned; `import_cli`, `layout_cli` red until `fixtures/code-consts/` is committed (their strict `git status` guard).
