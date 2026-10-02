---
class: canon
tier: 2
scope: [crates/specengine-core]
owner: owner
reviewed: 2026-10-02
---

# spec check: process rules

Check increment 4 (ADR-0031): each project declares its process rules (required keys, allowed values, labelled parts, own text) as `[[check.rules]]` under `[check]` of its `specengine.toml`. The core names no kind, key or label (`#universal`), only the four class names (ADR-0022), and builds no rule in: without `[[check.rules]]` no rule finding. One warning needs no rule: `parent-cycle`. Engine, verdict, output and debt: `docs/canon/spec-check.md`.

API (`specengine_core`): `CheckConfig.rules: Vec<CheckRule>` in the order written; `check::CheckRule {kinds, classes: Vec<DocClass>, paths, when, keys, values, parts, text: bool, severity, line}`, `when` and `values` as `Vec<(key, Vec<String>)>` by key; `own_spans(&ParsedFile, usize) -> Vec<Span>`, a node's own text, the one split (the store's `own_text` joins its pieces). Engine `check/rules.rs`, parsing `check/rules_toml.rs`, `parent-cycle` in `check/graph.rs`.

## A rule

```toml
[[check.rules]]                     # every question
kinds  = ["question"]
values = { status = ["open", "answered", "deferred", "dropped"], to = ["customer", "owner", "team"] }
text   = true
[[check.rules]]                     # an open question
kinds    = ["question"]
when     = { status = "open" }
keys     = ["to", "working_answer"]
parts    = ["Working answer"]       # a `**Working answer:**` lead-in or a heading
severity = "warning"                # default "error"
```

A rule needs a selector and a requirement; selectors AND, the items of a list OR.

- **`kinds`** select by the document's kind: its declared `kind:`, else the prefix kind of its `id:` (a declared `requirement` with an `assumption` prefix is a `requirement`). Each must be the kind of an `[ids]` prefix. **`classes`**: `class:`, each one of the four. **`paths`**: globs over the root-relative path, the `[paths] exclude` grammar.
- **`when`**: each key written as a string equal to one of its strings, exact (case-sensitive, no prefix match); absent, or not a string (a number, a list), the rule does not select.
- **`keys`**: each written (else `key-missing`) and not empty (else `key-empty`): null, blank, `[]`, `{}` are empty. Written keys come from the bytes as the class contract reads them (`title`, `kind`, `id` included).
- **`values`**: each written string of the key in its list, exact and case-sensitive; a list judged per item, one `value-invalid` per string outside; a number, boolean, map or a non-string item → `is not a string` (one per file and key); empty or absent → none (`keys` judges presence).
- **`parts`**: each label L present (else `part-missing`) and filled (else `part-empty`). L's place is the first in source order whose slug is L's (`crates/specengine-model/README.md` "Slug", any script, of the text alone, never de-duplicated), read by the core's pinned Markdown reader with the parse's options:
  - a heading of any level; its content runs to the next heading of the same or a higher level, nested headings' content included, their heading text not; `## X {#id} ##` is not matched;
  - a *lead-in*: strong emphasis (`**…**`, `__…__`) opening a paragraph, a list item, or a line inside one; its content runs from after it to the next lead-in line (any label) or the end of its paragraph or list item. Not lead-ins: anything in fenced, indented or HTML blocks, mid-line (`x **Cost.** y`), in link text or an image, in a heading, in a table cell, `***x***`; an italic or plain `Cost:` is no part.
  - *Filled*: a text or code run in the content, outside heading text and HTML (comments included), holds a letter or digit (`char::is_alphanumeric`). Whitespace, CRLF or a comment alone are empty.
- **`text = true`**: the own text is filled: the body minus nested ID sections (`own_spans`), minus headings, HTML and its comments, link and image text, wiki links (`[[…]]` on one line, code included), the parser's inline mentions (`R-12, A-101`); list markers never count; text in code spans and blocks does. "Regeneration waits for R-12." is filled; a body of links, headings and references is not.
- **`severity`**: `"error"` (default) or `"warning"`, for every finding of the rule.

**Judged files**: documents only; a `{#ID}` section never alone. Never a file whose front-matter failed nor a `class: generated` one; a Tier 3 document when selected (the shipped-features rule below judges Tier 3 specs). Empty `CheckFile.bytes` (a future index feed): keys, values and `when` come from the parse, `parts` and `text` judge nothing.

## Findings

| Code | Line | Subject | Message |
|---|---|---|---|
| `key-missing` (the class contract's) | 1 | key | ``key `K` is required by a check rule`` |
| `key-empty` | the key's | key | ``key `K` is empty; a check rule requires it filled`` |
| `value-invalid` | the key's | key | ``key `K` is `V`; allowed: a, b`` (as written), or ``key `K` is not a string`` |
| `part-missing` | 1 | L as written | ``part `L` is missing`` |
| `part-empty` | the heading's or lead-in's | L as written | ``part `L` is empty`` |
| `text-empty` | 1 | the document ID, else `""` | `no own text: only headings, links or references` |
| `parent-cycle` | the first member's `parent:` | the members | `` `parent:` forms a cycle through <subject> `` |

`CHECK_CODES` holds 35: the 29 before and the six above (`key-missing` reused). Lines and subjects never depend on a rule's position, so debt (code, path, subject) survives moved lines and reordered rules. Findings of one file equal in code, line, subject and message are one, an error over a warning; messages name no rule. A class contract and a rule requiring the same key give two `key-missing` lines (two messages); one debt entry covers both.

Severity, modes, debt, `(pre-existing)`: as every finding. A warning never blocks, counts in `warnings` and prints only with detail. Under `--staged` and `--changed` the base is judged with the checked `CheckConfig`, rules included (`docs/canon/spec-check-git.md` "The base"): with `enforce-introduced` a commit adding a rule is not blocked by violations `HEAD` already holds; a new one blocks. Rules never read task or queue state (`#control`, ADR-0012).

## parent-cycle

A warning, rules or none: one per `parent:` cycle `spec tree` breaks (`docs/canon/spec-cli-graph.md` "Parents and links") holding a live member (neither Tier 3 nor generated); a self-parent is one. The cycles come from `SpecGraph::cycles()` over a parents-only graph (the same parent resolution and breaking, no links), not built when no readable document gives `parent:`. Line: the `parent:` line of the first member in (path, position), the root `spec tree` lists. Subject: the member names (ID, else path) sorted by name, then path, joined `, ` (as `depends-cycle`). The members equal `spec tree`'s `warning:`; when that first member is `class: generated`, the check still reports at its line, while `spec tree` shows no warning (generated files are never admitted).

## Config errors

A malformed rule fails the whole `CheckConfig`: cannot check, exit 2 in every mode, reported in mode `enforce` (`docs/canon/spec-check-cli.md` "Cannot check"), `specengine.toml:<line>: message` at the offending value, else at the rule's `[[check.rules]]` header. `spec export index` refuses it too; read commands and the MCP server do not read `[check]`. The first error is reported: an unknown key (the TOML reader's message), then the fields in a fixed order (`kinds`, `classes`, `paths`, `when`, `keys`, `values`, `parts`, `severity`, then selector and requirement); in one `when` or `values` table, the first-written bad entry.

| Cause | Message |
|---|---|
| an empty list or table | ``check rule `kinds` is empty`` (each key alike) |
| a kind no prefix declares | ``check rule `kinds`: `X` is the kind of no `[ids]` prefix`` (skipped while `[ids]` is broken: the scheme reports it) |
| a class | ``check rule `classes`: `X` is none of canon \| decision \| spec \| generated`` |
| a glob | ``check rule `paths`: <problem>`` (the `[paths] exclude` checks) |
| a blank key name | ``check rule `keys`: an empty key name``, ``check rule `when`: an empty key name`` (`values` alike) |
| a `when`, `values` value | ``check rule `when` `K`: a string or a list of strings, not an integer``; ``… a list of strings, not one holding a table``; ``… an empty list`` |
| a label | ``check rule `parts`: label "…" has an empty slug (no letter, digit, `-` or `_`)`` |
| a severity | ``check rule `severity` `X` is not "error" or "warning"`` |
| no selector | ``check rule without a selector: give `kinds`, `classes` or `paths` `` |
| no requirement (`text = false` is none) | ``check rule without a requirement: give `keys`, `values`, `parts` or `text = true` `` |

## This repository

The root `specengine.toml`: every decision has a filled `Cost` part (the template's `**Cost.**` lead-in) and its own text; a shipped feature its "Implementation":

```toml
[[check.rules]]                     # decisions
kinds = ["decision"]
parts = ["Cost"]
text  = true

[[check.rules]]                     # shipped features
paths = ["docs/features/*.md"]
when  = { status = "shipped" }
parts = ["Implementation"]
```

Clean, 0 debt (`check_parity.rs`, PIN `[]`): a compacted shipped spec keeps its `## Implementation` summary.

## Cost and genre

Release, 5 000 documents each with `parent:`: the full `SpecGraph` ~340 ms, parents-only ~255 ms; 3 000 documents with 515 cycles 93 ms. Many unclosed `[[` on long lines: 180 → 14 ms (the `]]` search stops at the line end, kept between opens). The rules sources hold no kind of spec-a, spec-b or the root config, no key or label of the test configs, no project name (`check_genre.rs`); a Cyrillic label lives only under `fixtures/spec-b/` (ADR-0024).

## Open

- Own text over many nested ID sections is quadratic in them (16 000 alternating sections ~6 s, debug): pathological, the tier caps keep it out of reach.
- Placeholders ("—", "TODO") as unfilled: out of scope; a pilot's "no source" check needs them.
- The pilots' parity, at migration (ADR-0013, `docs/canon/spec-check.md` Q-8): cost-less ADRs → `severity = "warning"` or expiring debt (08 §4.2); non-English headers → the importer normalises first (08 §4.1).
- Not done: validating `kind:`; silencing `unknown-key` for keys a rule names; either-or requirements; type checks; rules on `{#ID}` sections; code rules and 08 AC-13 (Phase 3).
