---
class: canon
tier: 1
scope: [crates/specengine-model]
owner: owner
reviewed: 2026-09-29
---

# specengine-model — the corpus model and the reference grammar

Types and pure functions, `serde` only: no I/O, no parser library, no other SpecEngine crate in the normal graph (`specengine-eval` `tests/build_graph.rs`). Below `specengine-core` and `specengine-code`. Nothing here knows a project: prefixes, kinds and aliases come from `[ids]` (ADR-0008). The parser and its output contract: `crates/specengine-core/README.md`.

**Dev-only cycle.** `specengine-core` is a dev-dependency: `tests/grammar.rs` checks `canon:` and `status: superseded-by` through `parse`. Tests that use core live in `tests/` only (a `#[cfg(test)]` unit test would see a second copy of the model's types), and the edge never becomes a normal one (`cargo tree -e normal`).

## Modules

| Module | What it holds |
|---|---|
| `span` | `Span` `[start, end)`: byte offsets into the original file, BOM included |
| `script` | the look-alike table (fullwidth ASCII, Cyrillic and Greek letters identical to a Latin one); `IdScript` `latin` / `mixed` / `non-latin` (ADR-0009) |
| `scheme` | `IdScheme::new(vec![PrefixSpec::number(..), PrefixSpec::name(..).with_aliases(..)])`, `Shape`, `IdScope`, `SchemeProblem` / `SchemeError` |
| `grammar` | `scan` (text), `parse_reference` (one front-matter scalar), `parse_definition` (`id:`, `{#…}`), `parse_canon`, `split_superseded_by` |
| `reference`, `link`, `node`, `value`, `diagnostic`, `parsed` | what a parsed file is made of; `LINK_TYPES`, `MENTIONS`, `DiagnosticCode::ALL` (13 codes) |

## `[ids]` in `specengine.toml`

```toml
[ids]
R    = { kind = "requirement", width = 2, immutable_text = true }
Q    = { kind = "question",    width = 3, aliases_from = ["QST"] }
AC   = { kind = "criterion",   width = 2, scope = "feature" }
TERM = { kind = "term",        shape = "name" }
```

Prefix (the key): `[A-Z][A-Z0-9]*`, case-sensitive, Latin (ADR-0009; no `script` key). `kind`: required string, not validated (Q2 of the core README). `shape`: `"number"` (default) or `"name"` (`TERM-exhausted`, `RULE-STAM-REGEN`). `width` ≥ 1: required for `number`, forbidden for `name`; the zero-padded digit count `spec new` issues (`R-012` ≠ `R-12`), never checked on recognition. `aliases_from`: legacy prefixes in any script, `\p{L}[\p{L}\p{N}]*`, unique, never a prefix. `immutable_text`, `scope` (`"project"` | `"feature"`, cited from outside as `slug/ID`): carried only. Unknown key, bad prefix, missing or forbidden `width`, colliding alias → a scheme error `file:line: message`; the scheme loads whole or not at all.

## Reference grammar

```
reference = [ project ":" ] [ scope "/" ] id [ "#" id ] [ "@" rev ]
wiki      = "[[" reference [ "|" label ] "]]"
id        = prefix "-" body         ; configured prefix or aliases_from entry
body      = digit+                  ; shape "number"
          | alnum+ ( "-" alnum+ )*  ; shape "name", greedy; alnum = [A-Za-z0-9]
project   = slug ; scope = slug ; slug = [a-z][a-z0-9-]* ; rev = digit{1,9}
```

Examples: `R-12@3`, `slug/AC-07`, `shared:PAT-PROBES@3`, `MEC-STAMINA#RULE-STAM-REGEN`, `[[R-12|label]]`. One grammar for text, front-matter and markers.

Recognition: (1) candidate: a maximal letter-digit run followed by `-`, not preceded by `_` or `-` (`FOO-R-12` cites no `R-12`); (2) the run matches `aliases_from` verbatim first (→ `alias_of`, no `homoglyph`), else run and body match after look-alike normalisation, any normalised char giving `homoglyph` with the Latin `fix`; (3) `#` and `@` join only before an ID or 1–9 digits (`ADR-0002#layout` → `ADR-0002`); (4) right boundary: end, or a char that is no letter, digit, `_`, nor `-` before one (`R-12abc`, `R-12-3` cite nothing); (5) qualifiers by look-back, `slug/` then `slug:`, kept only if the char before the slug is none of letter, digit, `_ - . / :`, else dropped (`https://h.io/R-12` → `R-12`); (6) `script` of the ID as written: `latin`, `mixed` (ASCII letters plus a foreign letter or digit), `non-latin`. A `rev` out of `0..=999999999`, a `rev=` attribute that is not 1–9 digits, or `@` + digits that cannot be a revision → `bad-rev`, the reference kept without one.

Definitions (`id:`, `{#…}`) are a bare ID of a configured prefix, never an alias. A reference-carrying front-matter scalar is exactly one reference; `canon:` tries a reference, else `path[#anchor]`. Nothing is resolved here (existence, aliases, `slug/`, `project:`, anchors): that is `spec check`.

## Links

`LINK_TYPES`, closed and shared: `derived_from` (follows from a requirement or decision), `depends_on` (cannot be understood or built without), `constrains` (a rule restricts another), `supersedes` / `revises` (replaces / refines), `amends` (lives until applied), `answers` (a decision answers a question), `working_answer` (an assumption stands in for the answer), `uses_term`, `canon` (a decision promoted into canon; must resolve), `verifies` (a criterion or test verifies a node), `adopts` (a shared-library node at a pinned revision, 05 §3.6). Another declared type is kept with `unknown-link-type`. `MENTIONS` is the weak link of `refs`, `adrs` and every body reference; declared links are strong. `Link = {src?, src_span?, type, origin: frontmatter | inline, dst}`, `dst` a reference or a `canon:` path, unresolved. `parent` is containment, not a link (05 §8).

## Diagnostics

`Diagnostic = {code, severity, line, span?, fix?, message}`, `line` 1-based. Errors (part of the file unread, never fatal): `not-utf8` (no nodes), `frontmatter-unclosed` (all body), `frontmatter-yaml` (syntax: a line, no span; a cap exceeded), `frontmatter-not-mapping`, `frontmatter-type`, `id-not-in-scheme` (no node ID). Warnings: `unknown-key`, `unknown-link-type`, `unparsed-reference`, `homoglyph` (Latin `fix`), `kind-mismatch`, `duplicate-id` (in-file, both kept), `bad-rev`.

## Tests

`tests/grammar.rs`: table-driven splits of every example above, `canon:` path + anchor, `status: superseded-by` → `supersedes`.

## Open minors

- The look-alike table exists twice (here and `specengine-import` `script.rs`), and `specengine-code` `markers.rs` `is_latin_id` rejects `/`, `#`; census and markers switch to this crate in their own increments.
- An alias match containing a fullwidth digit still emits `homoglyph`.
- `OrderedMap` can emit duplicate JSON keys (`1` and `"1"`; `?` for collection keys).
