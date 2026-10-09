---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui, plugin]
ref: Phase 3 plan 2026-10-10, slice 2 (WA-1..WA-15, RD/TE/UI/SW criteria; review points decided under the owner's pre-approval, review open); 05 s3.3, s3.5; 08 s2 Phase 3
adrs: [ADR-0040]
---

# norm-hash: a node's meaning hash and revision

## Why

Slice 3 (`spec-lock`) will write each binding's `spec_hash` = the node's `norm_hash` into `spec.lock` (05 s3.5), slice 4 compares it, and a marker's `@N` (slice 1, `code-refs`) meets the node's revision. Neither contract exists: 05 s3.5 names what goes into the hash, not its bytes; a section's revision is the core README's open Q3, and today the parser accepts `rev=0` / `rev: 0` and takes the first of two `rev=`. Once a lock is committed the recipe cannot change without re-verifying every binding, so both are fixed before anything writes them.

The slice writes no spec, lock or file under a project root and blocks nothing (ADR-0012): one pure core function, one index column, one `spec show` field and its mirrors. ADR-0040 records the revision rule (`docs/canon/architecture.md#layout`).

## Description and interactions

- **An agent** sees ` | norm b3:<hex>` in `spec show`'s header and `norm_hash` in `get_node`; a proposal's base stays `span_hash` (WA-11, WA-12).
- **Slice 3** parses a blob's bytes and calls `norm_input`, then `b3_hash`; `revision`, `NORM_RECIPE` (WA-15).
- **Crates** (WA-9..WA-12): core the recipe and `revision`, no new dependency (`Cargo.lock`, `build_graph.rs` untouched); store the column; CLI `show`; MCP the mirror field; HTTP passes show's JSON through; UI the provisional type and mocks, no screen; plugin MINOR 0.7.0 (`get_node` returns a new key), no skill or role text change.
- **Unchanged**: `spec check` (no new code; `bad-rev` also covers `0` and a repeat), `bundle` text and `bundle_hash`, `tree`, `search`, model `Node`, the parse JSON, the reference grammar, `fixtures/daemon-keys.json` (NodeView lists top-level keys only).

## Data

### Revision

WA-1..WA-3, ADR-0040. A section's revision is its own `rev=N` attribute, a document's its front-matter `rev:`; never inherited.

| Written | Revision | Finding |
|---|---|---|
| nothing | 1 | none |
| `rev: 3`; `## Cost {#RULE-SPRINT-COST rev=3}` | 3 | none |
| `rev=007` | 7 (leading zeros read) | none |
| `rev=0`, `rev: 0` | unknown | `bad-rev` (new: today 0) |
| `rev=3a`, bare `rev`, 10 digits, `rev: -1` | unknown | `bad-rev` (as today) |
| `rev=2 rev=3` on one heading | unknown | `bad-rev` (new: today 2) |
| `rev:` twice in front-matter | unknown | YAML's own finding if it refuses the repeat, else `bad-rev` at the second |
| `rev: "3"`, `rev: [3]` | unknown | `frontmatter-type` (as today) |
| front-matter failed (`frontmatter-unclosed`, `frontmatter-yaml`, `frontmatter-not-mapping`) | unknown | that finding |
| a section without `rev=`, its document `rev: 5` | 1 | none |

`bad-rev` stays a warning (`engine.rs` severity table); no new code. The grammar is unchanged: `R-1@0` parses, and `@0` against any known revision is `spec_ahead` (`code-refs.md` "States").

Core: `revision(parsed: &ParsedFile, ord: usize) -> Revision`, `enum Revision {Written(u32), Unwritten, Unreadable}`, `number() -> Option<u32>` (`Unwritten` -> `Some(1)`, `Unreadable` -> `None`). Unreadable: a section whose `attrs` hold `rev` and whose `Node.rev` is none; a document whose `extra` holds `rev` or whose `front_matter_failure` is some. `show` and `tree` keep printing `rev` as written (` | rev <n>` only when written).

### The recipe

WA-4..WA-7, WA-13. The recipe sees only spans and links: no key, kind or prefix literal.

**Text.**

- A section: `own_spans(parsed, ord)`, its body (the line after its heading to its end) minus nested ID sections; never its heading line(s).
- A document (ord 0): `own_spans(parsed, 0)`, the file's body minus its ID sections, minus the first H1's line(s) with their line endings when that H1 is not an ID section. The H1 is found by re-scanning the body as the parser does (same `pulldown-cmark` options), never through slug anchors (an H1 of punctuation has none). A document's hash never covers its ID sections.

**Normalization**, in order: pieces joined with LF; CRLF, then a lone CR -> LF; trailing Unicode `White_Space` trimmed per line; runs of empty lines -> one; leading and trailing empty lines dropped; no final LF. Kept: indentation, tabs, inner spaces, Markdown, HTML comments, code blocks (the line rules apply inside them), the Unicode form (no NFC). A BOM is never text.

**Links** (documents only; a section's inline references are part of its text): `ParsedFile.links` with origin `frontmatter` and `src` equal to node 0's `id` (both absent included): every `links` item of any type (an unknown type as written), `supersedes`, `working_answer`, `canon`, `refs` and `adrs` as `mentions`. Not the `supersedes` link `status: superseded-by X` gives (its `src` is X). Each rendered from the parsed value, byte-sorted, duplicates dropped:

```
<type> ref:[<project>:][<scope>/]<id>[#<section>][@<rev>]     a reference: id Latin, an alias prefix as written,
                                                              rev decimal, wiki label and form dropped
<type> path:<path>[#<anchor>]                                 a canon: path target
```

**Never counted**: front-matter `id`, `kind`, `parent`, `title`, `status`, `tier`, `owner`, `reviewed`, `rev`, `class`, `scope`, `date`, `shipped`, `ref`, `to`, `severity`, `generator`, `source`, `acceptance`, `aliases`, `raised_by`, untyped (`extra`) keys; a heading line; a revision.

**Bytes** (framing as `specengine-hash/v2`, `crates/specengine-code/README.md` "Hash recipe"):

```
frame(x)  = u32le(len x) || x
input     = frame(NORM_RECIPE) || u32le(link count) || frame(link)... || frame(text)
NORM_RECIPE = "specengine-norm/v1"
norm_hash = b3_hash(input) = "b3:" + 64 lowercase hex BLAKE3          # specengine_store::b3_hash
```

Core: `pub const NORM_RECIPE`, `norm_input(parsed: &ParsedFile, bytes: &[u8], ord: usize) -> Option<Vec<u8>>`, `parsed` the parse of `bytes`; `None` = null (below). Pure: no I/O, no hash crate (WA-9).

Example, `fixtures/spec-a/docs/spec/movement/sprint.md`. RULE-SPRINT-COST (`## Cost {#RULE-SPRINT-COST rev=3}`, a nested `### Empty tank {#EDGE-SPRINT-EMPTY .playtest}` cut out):

```
12 00 00 00  "specengine-norm/v1"
00 00 00 00
3f 00 00 00  "Sprinting costs 12 units/s, set by R-12@2 and bounded by A-101."
```

MEC-SPRINT (`parent`, `class`, `tier`, `owner`, `reviewed` not counted; `# Sprint` and the Cost section cut; `## Tuning notes` has no ID, so it is text):

```
12 00 00 00  "specengine-norm/v1"
02 00 00 00
1e 00 00 00  "constrains ref:RULE-STAM-REGEN"
1a 00 00 00  "depends_on ref:MEC-STAMINA"
frame("Hold the sprint key to run; each second drains stamina (see\n[[MEC-STAMINA#RULE-STAM-REGEN|regeneration]]).\n\n## Tuning notes\n\nNumbers come from the feature stamina-tuning/AC-07 and from shared:DEC-0023@1.")
```

**Null** (WA-8): a document whose file carries `frontmatter-unclosed`, `frontmatter-yaml`, `frontmatter-not-mapping`, `frontmatter-type` or `unparsed-reference` (a declared link may be lost); its sections keep their hashes. A non-UTF-8 file has no nodes; `show`'s whole-file node is null. `id-not-in-scheme`, `homoglyph`, `bad-rev`, `unknown-*`, `duplicate-id` null nothing.

### Index

WA-10. `nodes.norm_hash TEXT` (NULL = null) on every node row, from the parse the row stores. `INDEX_FORMAT` 7 -> 8; `crates/specengine-store/tests/format_history.txt` gains one `8 <hash>` line after the seven; an index stamped 7 is rebuilt by the stamp check, the queue kept (`format.rs`). No read API yet: slice 3 reads it.

### spec show and mirrors

WA-11, WA-12.

```
RULE-SPRINT-COST | rule | Cost | docs/spec/movement/sprint.md:18 | <n> tokens | rev 3 | norm b3:<64 hex> | span b3:<64 hex>
```

- ` | norm <hash>` right before ` | span`, omitted when null; computed from a fresh parse of the bytes `show` read and prints (a document: the whole file), never the stored column; only the nodes printed are hashed. Only `shown_header` adds it: `header()`, shared with `bundle`, is unchanged.
- JSON node `{id, kind, title, path, line, end_line, status, rev, tokens_est, archived, utf8, sections, norm_hash, span_hash, text, truncated, omitted, links}`, `norm_hash` a string or `null`.
- MCP: `mirror.rs` `ShowNode.norm_hash: Option<String>` at that place; `get_node`'s outputSchema follows. HTTP: passed through. UI: `ShownNode.norm_hash: string | null` between `sections` and `span_hash` (provisional types, mocks).
- Plugin: `plugin.json` 0.7.0, a `PINS` entry (`docs/canon/plugin-files.md` "Version").

## Rules and edge cases

- WHEN the same bytes are parsed under the same `[ids]`, from any root, cwd or creation order, the system SHALL produce identical input bytes and hash (no path, root or time in the input) (WA-14, TE-9).
- WHEN text moves from a section into its parent, both hashes SHALL change; WHEN a nested section changes, the parent's SHALL NOT.
- WHEN an agent sends `norm_hash` as a proposal's `base`, the existing stale-base refusal names the current `span_hash`; nothing new is refused.
- Cost: linear in a node's own spans and links; the format bump forces one re-parse per worktree (WA-14).
- A project's own front-matter keys never count in v1; reflow and comment edits count (slice 4's `--editorial` is the escape). See "Open".

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- `NORM_RECIPE`, `norm_input` in core per "The recipe"; no Cargo change; `model_and_core_sources_do_no_file_io` (`build_graph.rs`) green. M: a `std::fs` call in the new module -> that test red.
- [ ] RD-2 -- `revision` and the parser per "Revision". M: `0..=` kept in `front_matter.rs` or `section_rev` -> `rev_zero_is_bad_rev`; first `rev=` wins -> `repeated_rev_is_bad_rev`.
- [ ] RD-3 -- the column and format 8; `show` text and JSON; the MCP mirror; plugin 0.7.0 + `PINS`.
- [ ] RD-4 -- nothing written under a root, no new finding code (`DiagnosticCode::ALL` 13, `CHECK_CODES` 35); `spec check` here as clean as before.

test-engineer:

- [ ] TE-1 -- golden: `norm_input` bytes (built by hand in the test, escapes for spec-b) and `b3:` hashes of MEC-STAMINA, RULE-STAM-REGEN, RULE-SPRINT-COST (spec-a), FLAG-DRY-RUN (spec-b). M: a final LF; links unsorted; big-endian lengths.
- [ ] TE-2 -- invariance, a row each, each naming its mutation: CRLF and lone CR; a BOM; trailing spaces; extra blank lines; a section heading renamed; a `rev` bump; the document's H1 renamed, a punctuation-only H1 too; each never-counted key changed; an `[ids]` kind or width change; links reordered, duplicated, `supersedes` moved between `supersedes:` and `links:`; a nested section edited, added, removed, moved (parent unchanged); another root or cwd.
- [ ] TE-3 -- sensitivity: a word; list nesting by indentation; a link added, removed, retargeted, its type or `@REV` changed; `working_answer` or `canon` changed; a subheading without ID renamed; text moved from a section into its parent changes both.
- [ ] TE-4 -- null: each of the five codes nulls the document (store NULL, no ` | norm`, JSON `null`), its sections keep theirs; a non-UTF-8 file: null, no ` | norm`. M: hashing the text alone on a broken front-matter.
- [ ] TE-5 -- every "Revision" row through `revision` and the parse; `R-1@0` parses; `bad-rev` a warning. M: 0 accepted; first `rev` wins; unreadable read as 1; a section inheriting its document's; `@0` made bad.
- [ ] TE-6 -- store: every node row of spec-a, -b, -c equals `b3_hash(norm_input(..))` of a fresh parse, NULL where null; `assert_equals_fresh` holds; `INDEX_FORMAT == 8` and the history the seven lines + one `8 ...` in both pins. M: the stamp kept at 7. A 7-stamped database rebuilds with its queue intact. M: a queue table dropped.
- [ ] TE-7 -- show: exact headers, norm before span; JSON keys and order; show = store = core over every ID of spec-a and spec-b; `bundle` text and `bundle_hash` byte-identical to before. M: `norm` added to `header()`.
- [ ] TE-8 -- mirrors: `mcp_read` parity and the outputSchema walker; HTTP parity; `fixtures/daemon-keys.json` byte-identical; `plugin_files` `PINS` 0.7.0.
- [ ] TE-9 -- determinism: two roots, another cwd, reverse creation order: identical hashes.
- [ ] TE-10 -- genre: spec-c hashed; a source scan of the recipe finds no fixture prefix, kind or key literal. M: a `"mechanic"` literal.
- [ ] TE-11 -- performance: a 50 000-section file within the `own_text` bound (store `rows.rs`). M: a quadratic join.
- [ ] TE-12 -- pilots, manual (`#[ignore]`, release, foreground): node, null and unreadable-revision counts; trees read-only; no pilot name or path committed.

ui-developer:

- [ ] UI-1 -- `ShownNode.norm_hash: string | null` between `sections` and `span_hash`; `provisional.test.ts` KEYS and CITED; mocks and builders give `b3:` + 64 hex. M: the key missing or after `span_hash`.

spec-writer:

- [x] SW-1 -- this spec.
- [x] SW-2 -- ADR-0040 and its `architecture.md#layout` diff in the same change.
- [ ] SW-3 -- at shipping: new Tier 2 `docs/canon/node-revision.md` (revision and recipe); core README (API, section `rev`, Q3 removed), model README (`rev`), store README (`INDEX_FORMAT`, `nodes`, Q4), CLI README `show`; `spec-cli-bundle.md` targets row; `mcp-read.md` `get_node`; 05 s3.3 and s3.5 "What goes into" pointers (the `rev` paragraph stays for slice 4); 08 s3 AC-13 note; `plugin-files.md` 0.7.0; `CLAUDE.md` State.
- [x] SW-4 -- `export index`, `check` clean.

## Risks

- The recipe is hard to change once slice 3 writes it (TE-1 pins it; a change is `specengine-norm/v2` and a re-lock).
- False changes accepted (reflow, comments, bullet style, NFC/NFD, an alias rewrite); false sameness avoided (indentation kept).
- Scheme coupling: a reference the scheme cannot parse nulls its document; an `[ids]` change re-parses by `scheme_fp`.
- Slice 1 (`code-refs`): `rev=0` becomes its `rev_unknown`, its `rev_state` calls `revision`; whichever lands second rebases.
- Test pins rewritten: store `format.rs` (format and history), `bundle_config.rs`, CLI `show.rs` headers and JSON, the MCP mirror, `ui/src/api/provisional.test.ts`; cap boundary tests may shift (~75 characters per shown node).
- Two hashes side by side; for a document `show` prints the whole file, `norm_hash` covers its own text and links only.

## Out of scope

`spec.lock`, `spec lock`, `spec bump`, `--editorial`, 08 AC-13's revision check; a finding for a meaning change without a bump; a `specengine.toml` list of counted project keys; section metadata attributes other than `rev`; NFC or reflow-insensitive text; a UI screen; `tree`, `search`, HTTP routes; writing `rev=` (only `apply_proposal`).

## Open

| Point | Answer |
|---|---|
| `rev: 0` / `rev=0` | `bad-rev`, unknown; `@0` stays legal, always `spec_ahead` |
| a project's own front-matter keys | never counted in v1 (later a `specengine.toml` list) |
| a document's hash and its ID sections | own text only (`architecture.md#layout`: sections are hashed one by one) |
| reflow, comment edits | counted in v1 |

Decided under the owner's pre-approval; the owner's review is open. WA-1..WA-15: the analysis' working answers (2026-10-10).

## Implementation

Not started. To record: modules, the rewritten pins, TE-12's figures, deviations with reasons.
