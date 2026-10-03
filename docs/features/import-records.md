---
class: spec
status: shipped
scope: [crates/specengine-import, crates/specengine-eval, fixtures, docs]
ref: pilots analysis 2026-10-03 split row 2, owner's answers Q1-Q9, census-config 2026-10-03, iteration-2 decisions 2026-10-04; 08 §4, §3 AC-6
shipped: 2026-10-04
adrs: []
---

# Import records: the record model and the "before" report

## Why

The census counts what a convention exposes, not whether every pilot record can be carried over. Findings (f)–(h) (08 §4.3) are invisible to it: A's criteria are list items, B's design specs open with a field/value table, B's principles have no hyphen; a losslessness check built on it would pass with thousands of records missing.

This task grew the census into a generic import engine (`specengine-import`): one import config per pilot (the census config extended, outside the repository; ADR-0008), a record model with verbatim hashes, a read-only "before" report; the after-tree is `import-layout`. Pilots A and B only. How it works now: `crates/specengine-import/README.md`.

## Acceptance criteria

Fixture ACs (decide "implemented"):

- [x] AC-01 A genre test over `crates/specengine-import/src` and the eval `import` module: no raw non-ASCII letter, Cyrillic escapes only in `script.rs`, no U+2116, no literal equal (case-insensitive) to a key, regex, map entry, prefix, separator or path of either fixture config or the generated one; every recognizer non-zero in both fixtures' `expected.json`. M: a numero-sign `local_number` default → red.
- [x] AC-02 Every new key accepted by `census` and `import`; an unknown key in each new table, a wrong type, a bad regex, each import regex as `''`, a `..` or absolute code root or base, a code root `.`, a legacy target outside `[A-Z][A-Z0-9]*` → exit 2 at `<config>:<line>`, nothing under `--out`. `census` on `corpus-mini` = its `expected.json`, on each new fixture = its run without the import keys. M: `deny_unknown_fields` dropped from `[lists]` → red.
- [x] AC-03 `import` on `fixtures/import-one`, `import-two`: `result` minus `*_ms` = each `expected.json`, pinning every `per_form` ≥ 2, `per_prefix`, `field_table` ≥ 1, `keys.mapped`, `keys.unmapped` ≥ 1, `values.mapped`, `local_number` ≥ 2, `none` ≥ 1, no record at those rows. M: the list-item recognizer off → red.
- [x] AC-04 Per fixture: an ID defined once, cited in a reference table and a reference path → 1 definition, 2 references, `duplicate_definitions` 0; a second definition outside them → 1; a feature-scoped ID defined in two documents → 0; a reference without definition → `unresolved` 1. M: the reference rule ignored → red.
- [x] AC-05 On a test-generated copy: a legacy-prefixed ID → Latin-prefixed `id`, `aliases` = [as written], `legacy.mapped` 1; an unmapped non-Latin prefix and an all-look-alike one → `legacy.unmapped` 2, no record, under an ASCII-only `like` too; each `hyphenless` match → `definitions` or `mentions`, no record, alias or unclaimed; a token two patterns match → once, under the first. M: an unmapped prefix mapped by look-alike guess; non-ASCII candidates found via `like`; a token counted per pattern → red.
- [x] AC-06 A planted ID-like token defined nowhere → `unclaimed` +1, one `unclaimed.json` line; `[lists]` removed → `list_item` 0, `unclaimed` up by ≥ the former `list_item` (`__ID__` items too); a `\b`-bounded `hyphenless` match inside `__…__` counts; a feature-scoped ID cited in its defining document → `claimed`, from another → only `feature_outside` +1 (cause `feature-outside`); a YAML scalar and list item citing a defined ID → `claimed` +2, an undefined one → `unclaimed` +1. M: list lines skipped by the scanner; feature-scoped tokens claimed from any document; front-matter values unscanned; an emphasis `_` breaking a bound → red.
- [x] AC-07 Hash unit tests per form: an inner byte changed (a space, `\|`, `<br>`) → another hash; CRLF, a cell's surrounding blanks → equal; a cell's hash = BLAKE3 of the trimmed cell; a comment on an item's first line stays in `text`, editing it → another hash; a lazy line in `text` verbatim, a heading, fence or item after it not; a `text_header` matching the ID-column header never yields the ID cell; a nested record item not in its parent's `text`, other nested lines in it; a `" -"` separator after `__ID__` not in `text`. M: inner whitespace collapsed; the first line comment-stripped; the lazy line dropped; the nested record in the parent's `text` → red.
- [x] AC-08 On each fixture (an excluded file, an unlisted extension, a dot-directory) `parse.files` = `import` `documents.total` = `census` `documents`; eval `parse.rs` has no `read_dir`. M: `parse` walking without `exclude` → red.
- [x] AC-09 `import --out` inside a fixture copy → exit 2, nothing created; stdout keys = the whitelist; a `[code]` root with one file citing a document twice and another once → `documents_cited` 2, `citations` 3; an unbounded `xa.md`, files under `target-x/`, `.cache/` uncounted; `git status --porcelain -- fixtures/` only `fixtures/import-*`; nextest `-p specengine-import -p specengine-eval`, clippy, fmt green; `git diff --exit-code -- Cargo.lock '*Cargo.toml'` empty. M: an ID on stdout → red.

Owner's check:

- [x] AC-10 (owner's check) Both import configs extended (legacy maps, feature prefixes, lead-ins, header maps, reference rules, local numbers, hyphenless patterns, link base, narrow code roots), `census` counts unchanged by the import keys; an `#[ignore]` test per pilot runs `import --label pilot-a|pilot-b` with only `SPECENGINE_PILOT_*`, `SPECENGINE_CENSUS_CONFIG_*` set: exit 0 within `--timeout`, the `tests/pilot` read-only proof over corpus and code roots equal, scratch `HOME` empty; `anonymity` green with `SPECENGINE_PILOT_A` / `_B`. "Implementation" records each pilot's non-zero counts, dated.

At shipping:

- [x] AC-11 Import README (config, record model, rules, report, census/import difference), eval README (`import`, walk minor gone) each ≤ 10 020 B; 08 §4.3 points at the import README; this spec compacted, AC numbers kept, `doc_pointers` green; `export index && check` clean; worst W ≤ 109 486 B.

## Implementation

Four iterations (the fourth owner-approved), review accepted; workspace 1 277 tests pass (15 skipped: pilot), eval 182; clippy, fmt clean; every named mutation red. No manifest or lock change. Canon: the import README (rewritten), eval README "CLI contract", "Pilot runs and tests"; 08 §2 Phase 1, §4.3. AC-11: import README 10 010 B, eval README 10 014 B, worst W 109 486 B.

| Module | What it does |
|---|---|
| import `walk.rs` | the one walk (census, import, eval `parse`); code-root files |
| `import/`: `mod.rs`, `lead_in.rs`, `ids.rs`, `header.rs`, `model.rs`, `code.rs` | `run`, `run_walked`; list-item extent; IDs, legacy, hyphenless; header maps; record model; code scan |
| `config.rs`, `markdown.rs`, `frontmatter.rs`, `census.rs` | import keys; comment ranges, emphasis blanks; front-matter values; census on the walk |
| eval `import.rs`, `main.rs`, `parse.rs` | the subcommand, whitelist, detail files; `parse` on the walk |

Tests: eval `import_cli.rs` (AC-02–06, 08–10; pilot tests by `tests/pilot` `run_census_read_only`), `import_genre.rs` (AC-01), `import_support/`; import `import_hash.rs` (AC-07), `import_rules.rs`. Fixtures `import-one` (requirements, the default), `import-two` (decision log): invented, Latin-script; non-Latin input built from escapes at test time.

Iterations: 1 the engine; 2 first-line comments kept, separators past blanks, unmapped candidates independent of `like`, lazy lines, nested-record stop, front-matter values, `feature_outside`, emphasis blanks, hyphenless once; 3 siblings at four columns or a tab end an item, blank as written, comment continuation, YAML quotes only at a scalar start; 4 fences in items tracked to their close, no block test on comment continuations, quotes after `!tag` / `&anchor`.

Deliberate readings: a lazy line also stops at a block quote and a `<!--` line start (CommonMark); block syntax is read past the marker's columns, not the content column; both header forms count `yaml`, a skipped document `none`; `kept` = a `key_map` target or `value_map` table; hyphenless in front-matter values are mentions; empty `code.extensions` = every file; `id_header` may match `""` (a census key); the shared quote-aware strip cuts a census `class: it's # x` at the comment; detail entries but records in processing order.

**AC-10**, 2026-10-04, dev profile, exit 0:

| | A | B |
|---|---|---|
| documents (= `parse` files = `census`) | 375 | 150 |
| records: table, headerless, list | 2 940: 2 153, 415, 372 | 558: 538, 20, 0 |
| definitions; references (unresolved); duplicates | 1 325; 1 615 (8); 37 | 261; 297 (0); 28 |
| rows without ID: local, none | 18, 0 | 155, 12 |
| yaml, field table, none; non-Latin keys | 375, 0, 0; 0 | 63, 41, 46; 104 |
| keys mapped, unmapped; values mapped, unmapped | 0, 1 929; 0, 0 | 394, 321; 84, 19 |
| legacy mapped; hyphenless definitions, mentions | 97; 0, 0 | 287; 269, 3 222 |
| claimed, unclaimed, feature-outside; files | 37 092, 427, 5 968; 120 | 5 863, 178, 0; 43 |
| wiki, resolved by base; code files, cited, citations | 1, 57; 197, 92, 911 | 143, 0; 193, 16, 44 |
| census ID rows, without ID, record tables, non-Latin, duplicates | 2 568, 70, 130, 19, 1 383 | 343, 185, 56, 287, 110 |

B's census is unchanged; A's moved only by the owner-settled `id_header`, a census key (was 2 593, 9, 123, 20, 1 407). Read-only: proofs equal, each pilot's `git status` equal, scratch `HOME`s empty; `anonymity` green with both variables.

## Open

Follow-up `import-gaps`, before `import-layout`:

1. A's criteria lead-ins hold "ID. Title" inside the strong span: 135 of ~904 become records, the rest `feature_outside` (a lead-in rule change).
2. IDs defined outside record positions (front-matter `id:`, a mapped header key, the file name) are no definitions: A 226, B 173 unclaimed.
3. Code-scan `strip` credits a deeper relative path to a shallower same-name document.
4. Identity or Latin header keys count `unmapped`.
5. A record line right after `-->` on the same line also joins the previous record's text.
6. `end --> # y` ends an item though `end --> - x` continues it.

Notes: the census still normalises all-look-alike prefixes; fence openers and closers in items are measured past the marker's columns (openers at content +2/+3 untracked); `classify_lines` misses fences at ≥ 4 columns. Finding (h) needs a reference-grammar change and an ADR superseding ADR-0009 before B migrates. A's criteria cited from outside their feature (`slug/ID`; meanwhile `feature_outside`): `import-layout`.
