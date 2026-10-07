---
class: canon
tier: 1
scope: [crates/specengine-model]
owner: owner
reviewed: 2026-09-30
---

# specengine-model — the corpus model and the reference grammar

Types and pure functions, `serde` only: no I/O, no parser library, no other SpecEngine crate in the normal graph (`specengine-eval` `tests/build_graph.rs`). Below `specengine-core` and `specengine-code`. Nothing here knows a project: prefixes, kinds and aliases come from `[ids]` (ADR-0008). The parser and its output contract: `crates/specengine-core/README.md`.

## Modules

| Module | What it holds |
|---|---|
| `span` | `Span` `[start, end)`: byte offsets into the original file, BOM included |
| `script` | the look-alike table (fullwidth ASCII, Cyrillic and Greek letters identical to a Latin one); `IdScript` `latin` / `mixed` / `non-latin` (ADR-0009) |
| `scheme` | `IdScheme::new(vec![PrefixSpec::number(..), PrefixSpec::name(..).with_aliases(..)])`, `Shape`, `IdScope`, `SchemeProblem` / `SchemeError` |
| `grammar` | `scan` (text), `parse_reference` (one front-matter scalar), `parse_definition` (`id:`, `{#…}`), `parse_canon`, `split_superseded_by`, `is_slug` (the `slug` rule: qualifiers, feature-document stems) |
| `task` | `TaskStatus` (ten states), `RunOutcome`, `TaskPackage` (25 keys, `TASK_PACKAGE_SCHEMA_VERSION` 1) and its parts, types only: `docs/canon/task-package.md` |
| `reference`, `link`, `node`, `value`, `diagnostic`, `parsed` | what a parsed file is made of; `Anchor`, `AnchorOrigin`, `LINK_TYPES`, `MENTIONS`, `DiagnosticCode::ALL` (13 codes) |

## `[ids]` in `specengine.toml`

```toml
[ids]
R    = { kind = "requirement", width = 2, immutable_text = true }
Q    = { kind = "question",    width = 3, aliases_from = ["QST"] }
AC   = { kind = "criterion",   width = 2, scope = "feature" }
TERM = { kind = "term",        shape = "name" }
```

Prefix (the key): `[A-Z][A-Z0-9]*`, case-sensitive, Latin (ADR-0009; no `script` key). `kind`: required string, the project's vocabulary, not validated (ADR-0031). `shape`: `"number"` (default) or `"name"` (`TERM-exhausted`, `RULE-STAM-REGEN`). `width` ≥ 1: required for `number`, forbidden for `name`; the zero-padded digit count `spec new` issues (`R-012` ≠ `R-12`), never checked on recognition. `aliases_from`: legacy prefixes in any script, `\p{L}[\p{L}\p{N}]*`, unique, never a prefix. `immutable_text`: carried only. `scope` (`"project"`, default | `"feature"`: defined in a feature document, cited from outside as `slug/ID`): enforced by `spec check` (`docs/canon/spec-check-links.md`). Unknown key, bad prefix, missing or forbidden `width`, colliding alias → a scheme error `file:line: message`; the scheme loads whole or not at all.

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

Recognition: (1) candidate: a maximal letter-digit run followed by `-`, not preceded by `_` or `-` (`FOO-R-12` cites no `R-12`); (2) the run matches `aliases_from` verbatim first (→ `alias_of`, no `homoglyph` even for a look-alike body digit; `id` = the written prefix + the normalised body), else run and body match after look-alike normalisation, any normalised char giving `homoglyph` with the Latin `fix`; (3) `#` and `@` join only before an ID or 1–9 digits (`ADR-0026#layout` → `ADR-0026`); (4) right boundary: end, or a char that is no letter, digit, `_`, nor `-` before one (`R-12abc`, `R-12-3` cite nothing); (5) qualifiers by look-back, `slug/` then `slug:`, kept only if the char before the slug is none of letter, digit, `_ - . / :`, else dropped (`https://h.io/R-12` → `R-12`); (6) `script` of the ID as written: `latin`, `mixed` (ASCII letters plus a foreign letter or digit), `non-latin`. A `rev` out of `0..=999999999`, a `rev=` attribute that is not 1–9 digits, or `@` + digits that cannot be a revision → `bad-rev`, the reference kept without one.

Definitions (`id:`, `{#…}`) are a bare ID of a configured prefix, never an alias. A reference-carrying front-matter scalar is exactly one reference; `canon:` tries a reference, else `path[#anchor]`. Nothing is resolved here (existence, aliases, `slug/`, `project:`, anchors): that is `spec check` (`docs/canon/spec-check.md`).

## Links

`LINK_TYPES`, closed and shared: `derived_from` (follows from a requirement or decision), `depends_on` (cannot be understood or built without), `constrains` (a rule restricts another), `supersedes` / `revises` (replaces / refines), `amends` (lives until applied), `answers` (a decision answers a question), `working_answer` (an assumption stands in for the answer), `uses_term`, `canon` (a decision promoted into canon; must resolve), `verifies` (a criterion or test verifies a node), `adopts` (a shared-library node at a pinned revision, 05 §3.6). Another declared type is kept with `unknown-link-type`. `MENTIONS` (`is_weak_link`) is the weak link of `refs`, `adrs`, every body reference and every local Markdown link; declared links are strong. `Link = {src?, src_span?, type, origin: frontmatter | inline, dst}`, `dst` a reference or a path, unresolved: `PathTarget {path, anchor?, span?}` is a `canon:` value or a Markdown link destination (split and `span`: `docs/canon/spec-check-links.md` "File links"). `parent` is containment, not a link.

Walks (`Direction`; `docs/canon/spec-cli-graph.md`): `graph_direction` follows every strong type out; `IMPACT_LINK_TYPES` (`impact_direction`): `depends_on`, `derived_from`, `verifies`, `uses_term` in, `constrains` out.

## Anchors

`Anchor {name, origin: AnchorOrigin, level?, span}`: where a `path#name` link can land, e.g. `{"name":"license","origin":"slug","level":2,"span":[1402,1412]}`. `ParsedFile.anchors` holds them in source order, a heading's `slug` before its `attr`. Origins: `slug` — every heading's GitHub slug; `attr` — a `{#…}` that is no definable ID (a section's own ID is its node); `html` — `<a id="…">`, `<a name="…">` outside code and HTML comments (`<!-->`, `<!--->` are whole comments, CommonMark 0.31). `slug`, `attr`: `level` and span = the heading line(s); `html`: span = the start tag `<`…`>`, no level.

**Slug** (github-slugger; computed by core): the heading's inline text (text, code, link text; no destination, image, HTML), `{#…}` removed, lowercased; letters and digits of any script, `-`, `_` kept; whitespace → `-`; the rest dropped; a repeated base gets `-1`, `-2`, … skipping slugs already given; empty → no anchor. Accepted divergences: GitHub differs on two-line setext headings, NBSP and tab (`-` here), combining marks (dropped here), non-`#` `{…}` blocks (pulldown-cmark strips them).

## Diagnostics

`Diagnostic = {code, severity, line, span?, fix?, message}`, `line` 1-based. Errors (part of the file unread, never fatal): `not-utf8` (no nodes), `frontmatter-unclosed` (all body), `frontmatter-yaml` (syntax: a line, no span; a cap exceeded), `frontmatter-not-mapping`, `frontmatter-type`, `id-not-in-scheme` (no node ID). Warnings: `unknown-key`, `unknown-link-type`, `unparsed-reference`, `homoglyph` (Latin `fix`), `kind-mismatch`, `duplicate-id` (in-file, both kept), `bad-rev`.

## Tests

`tests/grammar.rs`: table-driven splits of every example above; `canon:` path + anchor, `status: superseded-by` → `supersedes` through core's `parse`. **Dev-only cycle**: core is a dev-dependency, so tests using it live in `tests/` (a `#[cfg(test)]` one would see a second copy of the model's types); never a normal edge (`cargo tree -e normal`).

## Open minors

- The look-alike table exists twice (here and `specengine-import` `script.rs`) → the importer increment; `specengine-code` `markers.rs` `is_latin_id` rejects `/`, `#` → the marker-parser increment: both then use this crate.
- `see:R-12` in prose reads as project `see` → the `project:` increment.
- A name-shape body is greedy (`MEC-STAMINA-based` is one ID); the check's `Resolver` falls back (`docs/canon/spec-check-graph.md`).
