---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui]
owner: owner
reviewed: 2026-10-14
---

# Node revision and meaning hash

A node's **revision** is what a marker's `@N` (ADR-0018) and a lock entry's `rev` meet; its **meaning hash** `norm_hash` is what `spec.lock`'s `spec_hash` holds (05 §3.5). Both are pure core functions of one parse (`crates/specengine-core/README.md` "API"): no file is written, nothing is blocked (ADR-0012). The rule is ADR-0040 (`docs/canon/architecture.md#layout`). The recipe is frozen as `specengine-norm/v1`: any change to the bytes below is `specengine-norm/v2` and a re-lock; `crates/specengine-core/tests/norm_recipe.rs` pins v1 (golden inputs and hashes).

## Revision

A document's revision is its front-matter `rev:`, a section's the `rev=N` attribute of its own `{#ID ...}` heading; never inherited. 1 to 999 999 999; none written is 1; unknown is never 1.

| Written | Revision | Finding |
|---|---|---|
| nothing (also a section in a `rev: 5` document) | 1 | none |
| `rev: 3`; `## Cost {#RULE-SPRINT-COST rev=3}` | 3 | none |
| `rev=007` | 7 (leading zeros read) | none |
| `rev=0`, `rev: 0`, `rev=3a`, bare `rev`, 10 digits, `rev: -1` | unknown | `bad-rev` |
| `rev=2 rev=3` on one heading | unknown | `bad-rev` |
| `rev:` twice in front-matter | unknown | `frontmatter-yaml` (the YAML reader refuses the repeat), else `bad-rev` at the second |
| `rev: "3"`, `rev: [3]` | unknown | `frontmatter-type` |
| front-matter failed (`frontmatter-unclosed`, `-yaml`, `-not-mapping`) | unknown | that finding |

Core `revision(parsed, ord) -> Revision {Written(u32), Unwritten, Unreadable}`, `number()`: `Written(n)` n, `Unwritten` 1, `Unreadable` none. **Unreadable**: no node at `ord`; a section whose heading `attrs` hold `rev` and whose `Node.rev` is none; a document whose front-matter failed (`front_matter_failure`) or whose `extra` holds `rev` (a bad, repeated or mistyped value is kept there); a `Node.rev` outside the range (a parse of another format). The model's `Node.rev` holds only a written, readable revision: `spec show` and `tree` print ` | rev <n>` only then, JSON `rev` is `null` otherwise.

`bad-rev` stays a warning, no new code. The reference grammar is unchanged: `R-1@0` parses and is no finding.

## Raising

ADR-0043 (`architecture.md#layout`, `#apply`), built by slices 4b-1 (approval, shipped 2026-10-14; as built: `docs/canon/proposal-revisions.md`) and 4b-2 (`docs/features/rev-rule.md`: the rule, `spec bump`; until it ships no check reads `spec.lock`).

- **Approval**: `apply_proposal` raises, at step 6, the revision of each node with an ID whose `norm_hash` its change alters, from the file as read at apply, never lowering one; the question and a `Revision:` trailer name it. A meaning approve is refused before the prompt for a node at `REV_MAX`, of a closed class admitting no `rev` or whose raise cannot be written; an unreadable revision refuses any approve. `spec approve --editorial`, the owner's choice, keeps every revision and, in the same commit, sets the new `norm_hash` on that node's `spec.lock` entries that vouched for the old text at that revision (`Lock-mode: editorial`). A proposal whose text changes a revision is refused at creation: approval sets revisions.
- **The rule**: `spec check --staged` and `--changed`, never plain, give one error per node a `HEAD` lock entry vouches for (at its `HEAD` revision, `spec_hash` = its `HEAD` `norm_hash`) whose `norm_hash` changed with neither a raise nor that refresh in the checked tree; `[check] mode` decides whether it blocks. Unlocked nodes, `@N` pins and commits skipping the hook are not judged.
- **`spec bump`** only lists nodes changed without a raise, `required` (the rule's) or `suggested`; it writes nothing.

## The recipe

It sees spans and links only: no key, kind or prefix literal (a source scan in the tests).

**Text** (`own_spans`, as the store's `own_text`). A section: its body (the line after its heading to its end) minus nested ID sections; never its heading line(s). A document (ord 0): the file's body minus its ID sections, minus its first H1, from the start of the H1's first line through the line ending of its last, when that H1 is no ID section. The H1 is found by re-reading the body with the parser's reader and options (`markdown::first_h1`), never through slug anchors (a punctuation-only H1 has none). The cut leaves the text before and after it as two pieces. A document's hash never covers its ID sections; each has its own.

**Normalisation**, in order:

1. in each piece, CRLF, then a lone CR, to LF (per piece, before the join: a piece ending in a lone CR stays a line apart from the next, as in the file's LF form);
2. the pieces joined with LF: `Intro\n# Title\nBody` hashes as `Intro\n\nBody`;
3. trailing Unicode `White_Space` trimmed per line;
4. runs of empty lines to one, leading and trailing empty lines dropped, no final LF.

Kept: indentation, tabs, inner spaces, Markdown, HTML comments, code blocks (the line rules apply inside them), the Unicode form (no NFC). A BOM is never text.

**Links**, a document's only (a section's inline references are part of its text): `ParsedFile.links` with origin `frontmatter`, no `src_span`, `src` equal to node 0's `id` (both absent included): every `links` item of any type (an unknown type as written), `supersedes`, `working_answer`, `canon`, and `refs`, `adrs` as `mentions`. Never the `supersedes` link of `status: superseded-by X` (`src` X, `src_span` set), even when X is the document itself. Each rendered from the parsed value, byte-sorted, duplicates dropped:

```
<type> ref:[<project>:][<scope>/]<id>[#<section>][@<rev>]   the ID Latin (an alias prefix as written), rev decimal,
                                                            a wiki label and form dropped
<type> path:<path>[#<anchor>]                                a canon: path target
```

**Never counted**: every other front-matter key, typed (`id`, `kind`, `parent`, `title`, `status`, `tier`, `owner`, `reviewed`, `rev`, `class`, `scope`, `date`, `shipped`, `ref`, `to`, `severity`, `generator`, `source`, `acceptance`, `aliases`, `raised_by`) or untyped (`extra`); a heading line; a revision.

**Bytes**, framed as `specengine-hash/v2` (`crates/specengine-code/README.md` "Hash recipe"):

```
frame(x)    = u32le(len x) || x
input       = frame(NORM_RECIPE) || u32le(link count) || frame(link)... || frame(text)
NORM_RECIPE = "specengine-norm/v1"
norm_hash   = b3_hash(input) = "b3:" + 64 lowercase hex BLAKE3        # specengine_store::b3_hash
```

Core `NORM_RECIPE`, `norm_input(parsed, bytes, ord) -> Option<Vec<u8>>`, `parsed` the parse of `bytes`, `None` null. Pure: no I/O, no hash crate. Linear in the node's own pieces and links: only they are read as UTF-8 (a document also its body, once), never the whole file per node; 50 000 sections of one file in ~0.15 s (debug).

Example, `fixtures/spec-a/docs/spec/movement/sprint.md`. RULE-SPRINT-COST (`## Cost {#RULE-SPRINT-COST rev=3}`, its nested `### Empty tank {#EDGE-SPRINT-EMPTY .playtest}` cut), `b3:bb97b1d5…1623ca4d`:

```
12 00 00 00  "specengine-norm/v1"
00 00 00 00
3f 00 00 00  "Sprinting costs 12 units/s, set by R-12@2 and bounded by A-101."
```

MEC-SPRINT (`parent`, `class`, `tier`, `owner`, `reviewed` not counted; `# Sprint` and the Cost section cut; `## Tuning notes` has no ID, so it is text), `b3:1a1fd131…6f5cdcae`:

```
12 00 00 00  "specengine-norm/v1"
02 00 00 00
1e 00 00 00  "constrains ref:RULE-STAM-REGEN"
1a 00 00 00  "depends_on ref:MEC-STAMINA"
frame("Hold the sprint key to run; each second drains stamina (see\n[[MEC-STAMINA#RULE-STAM-REGEN|regeneration]]).\n\n## Tuning notes\n\nNumbers come from the feature stamina-tuning/AC-07 and from shared:DEC-0023@1.")
```

## Null

`norm_input` is `None` and the hash null for:

- a document whose file carries `frontmatter-unclosed`, `frontmatter-yaml`, `frontmatter-not-mapping`, `frontmatter-type` or `unparsed-reference` (a declared link may be lost); its sections keep theirs;
- no node at `ord`: a non-UTF-8 file has none, so `show`'s whole-file node of one is null;
- a span that is no UTF-8 range of `bytes` (a parse of other bytes); a piece or link over 4 GiB.

`id-not-in-scheme`, `homoglyph`, `bad-rev`, `unknown-*`, `duplicate-id` null nothing.

## Where it appears

- Index: `nodes.norm_hash` after `own_text`, NULL when null, from the parse the row stores (`INDEX_FORMAT` 8: `crates/specengine-store/README.md`). No read API yet; `spec lock accept` hashes the node's `HEAD` blob itself (`spec-lock-accept.md`).
- `spec show`: ` | norm b3:<hex>` right before ` | span`, omitted when null; JSON `norm_hash` between `sections` and `span_hash`, a string or `null` (`crates/specengine-cli/README.md` "Output and the cap"). From a fresh parse of the bytes printed, never the stored column: a document's covers its own text and links, its `span_hash` the whole file printed.
- `spec bundle`'s headers carry neither norm nor span; its text and `bundle_hash` did not change.
- The same key in MCP `get_node` (`docs/canon/mcp-read.md`), the daemon's show JSON and the UI's provisional `ShownNode.norm_hash`. A proposal's base stays `span_hash`: a `norm_hash` sent as `base` meets the stale-base refusal naming the current `span_hash` (untested).

Determinism: the same bytes under the same `[ids]` give the same input from any root, cwd or creation order; an `[ids]` kind or width change keeps every input.

## Known limits

- Lone-CR line endings: a lone CR ending a heading line or a front-matter fence changes the parse (the fence needs LF; pulldown-cmark ends no ATX heading at a lone CR: `# Title\rBody\r` is one heading), so such a file hashes unlike its LF form; a CR inside text is plain LF.
- A `status: superseded-by <the document's own ID>` written with YAML escapes has no verbatim `src_span` and counts as a `supersedes` link.
- False changes accepted: reflow, comment edits, bullet style, NFC against NFD, an alias prefix rewritten (Phase 3's `--editorial` is the escape); false sameness avoided (indentation kept).
- A project's own front-matter keys never count in v1 (later a `specengine.toml` list, so v2).
- A reference the scheme cannot parse nulls its document.
