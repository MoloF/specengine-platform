---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-model]
owner: owner
reviewed: 2026-10-02
---

# spec check: feature scopes and links

Increment 2 part 2 of `spec check`, two passes, both shipped 2026-09-30: A, feature scopes (ADR-0026, `docs/canon/architecture.md#layout`); B, Markdown file links ("File links"; 08 §4.3 (a), modelled on the census resolver). Engine, config, verdict, output: `docs/canon/spec-check.md`; index render and graph warnings: `docs/canon/spec-check-graph.md`. A scope comes only from `[paths] features` and `[ids] scope`: no prefix, slug, `features/`, `records/` or `docs/` literal in the sources (ADR-0008, `check_genre.rs`).

## Configuration

```toml
[paths]
features = "docs/features"   # default; its direct *.md children with a slug stem are feature documents
[ids]
AC = { kind = "criterion",   width = 2, scope = "feature" }
R  = { kind = "requirement", width = 2 }                  # scope = "project", the default
```

## Feature documents

A walked `.md` file directly under `[paths] features` whose stem satisfies `grammar::is_slug`; its slug is the stem. Class, tier, status and a failed front-matter do not matter: a shipped (Tier 3) feature still defines its IDs. With `features = "specs/feat"`, `specs/feat/a.md` is one; `specs/feat/sub/b.md` (nested), `specs/feat/Upper.md` and `specs/feat/README.md` (stem no slug), an excluded or unwalked file, `docs/features/c.md` are not.

```markdown
# Stamina tuning                                     <- docs/features/stamina-tuning.md

Tune the regeneration delay and record the result in AC-07.   <- bare: this file's section

### Regeneration starts 1.5 s after the last sprint {#AC-07}

Verifies R-12. A sprint followed by rest shows the first regeneration tick 1.5 s later.
```

Other files cite it `stamina-tuning/AC-07`, a section of that file `stamina-tuning/AC-07#AC-08`.

## Definitions and uniqueness

An ID is feature-scoped when its prefix has `scope = "feature"`; a reference's legacy prefix counts through its `aliases_from` target (a definition is never an alias). A feature-scoped ID is defined only as a `{#ID}` section of a feature document. A document `id:` (any file, feature documents included) or a `{#ID}` section elsewhere → **`id-scope`** (error), one per definition, on every walked file of any class or tier, at the `id:` line or the heading, subject the ID:

    error  docs/records/AC/AC-07.md:2: id-scope: `AC-07` is feature-scoped: define it as a `{#AC-07}` section of a document directly under `docs/features`

That definition gets no `id-taken` and no `file-name` (one finding per defect); `id-width` stays. Uniqueness is per feature: two features may define the same ID, never `id-taken`; a repeat inside one file is the parser's `duplicate-id` (error). Project-scoped IDs: `id-taken` as before.

## Resolution

Every resolution — front-matter references (`ref-dangling`), inline mentions (`mention-dangling`), `depends-cycle` edges, `ref-superseded` — goes through the `Resolver`, with the citing file. The ID, `aliases:`, `aliases_from` and `#Y` rules of `docs/canon/spec-check.md` "Rules" apply inside the place:

| Written | Citing file | Resolves when |
|---|---|---|
| `feat/AC-01`, `feat/R-97` (any prefix) | any | `<features>/feat.md` is a feature document and defines the ID |
| `feat/AC-01#AC-02` | any | as above, and `AC-02` is a section of that file |
| `AC-01` (feature-scoped, bare) | feature document F | F defines it |
| `AC-01` (feature-scoped, bare) | any other | never |
| `R-12` (project-scoped, bare) | any | defined anywhere, as before |
| `AC-01` (feature-scoped, bare) | none (`spec show`) | defined anywhere, as `R-12` |
| `other:feat/AC-98` | any | `Skipped`, no finding (`project:`: a later increment) |

"Defines" = a document `id:` or a `{#ID}` section of that one file. `@rev` is not checked. The inline name-shape fallback (`docs/canon/spec-check-graph.md`) retries in the same place. Unresolved: inline → `mention-dangling` (warning), front-matter → `ref-dangling` (error), message `` `<key>`: `<written>` <reason> ``, the reason computed for the ID as written, never for a fallback candidate:

| Case | Reason |
|---|---|
| no feature document at the path | ``resolves to no feature document `specs/feat/feat.md` `` |
| the file lacks the ID | ``is not defined in `specs/feat/feat.md` `` |
| the file lacks `#Y` | ``has no section `#AC-02` in `specs/feat/feat.md` `` |
| bare, defined in features | ``is feature-scoped: cite it as `feat/AC-01` `` (several: `` or `other/AC-01` `` each, path order) |
| bare, defined in no feature | `is feature-scoped and no feature document defines it` |

A reference without a span gets `<written>` rebuilt as `project:slug/ID#Y` (message text only).

**Graph rules.** A `depends_on` item resolves from its own file: a scoped item gives an edge to its feature document, a bare feature-scoped one only inside its own feature. `ref-superseded` is unchanged, with scoped resolution. Findings, cycle subjects and reasons do not depend on input order.

**`parent:`.** The model's `ParentRef` keeps only the ID and a span; the check re-parses the verbatim bytes under the span, so `parent:` keeps its `project:`, `slug/` and `#section` like any front-matter reference: its `#Y` is checked, `parent: other:X` is `Skipped`. Limit: an escaped (non-verbatim) value has no span, so its qualifiers and section are lost — `parent: "feat\/AC-01"` outside a feature gives a false `ref-dangling` (open question below).

## API

- `specengine_core::check::resolve`: `Resolver::new(&CheckInput, &IdScheme, &Paths)`, blind to input order; `resolve(from, &Reference, written)` and `resolve_mention(from, &Reference, written)` (with the name fallback) → `Resolution`; `resolve_detached(&Reference, written)`: no citing file (the table), no fallback; `holders_of(from, &Reference, written) -> Option<Vec<usize>>` (the ID's files, section ignored, no fallback; `None`: dangling or skipped); `from` = the citing file's path (`""` or an unwalked path: no feature document); `feature_slug(path) -> Option<&str>`; `paths()`, what `Resolved` indexes. `Resolution::{Resolved(Vec<usize>), Skipped, Dangling(reason)}`, `Skipped` = `project:` only.
- `specengine_model::grammar::is_slug(&str) -> bool`: the grammar's `slug`, `[a-z][a-z0-9-]*`, one rule for the lexer's qualifiers and feature stems.
- `specengine_core` root: `Paths.link_base`; `WalkScope` (`Paths::walk_scope()`, `new(&Paths)`), the walk's rules without the disk, `exclude` compiled once — `roots()`, `is_excluded`, `in_walk_scope` — shared by the store's walker and the link check (`Paths::is_excluded`, `in_walk_scope` compile per call); `is_under(path, dir)`, `is_clean_relative`; `DOCUMENT_EXTENSION` = `.md`.
- `CHECK_CODES` gains `id-scope`, `link-dangling`, `link-anchor`.

## This repository and the template

The parity config has only `ADR`, project-scoped, and no `link_base`: `docs/features/*.md` are feature documents defining nothing scoped; `enforce` stays `clean` with its one pinned warning, no link finding. `docs/features/_template.md` writes criteria as list items, right while `AC` is unconfigured here. **Open item:** a project configuring a feature-scoped `AC` needs `{#AC-01}` criterion sections, so the template and the Phase 2 role prompts switch first (with the root `specengine.toml`, Q-7); a criterion section has no front-matter (Q-I).

## File links

**Recorded** by the parser: each inline link and reference definition (used or not; a repeated label: the first) with a local destination — non-empty, not `//`-led, no scheme `[A-Za-z][A-Za-z0-9+.-]*:` — as one `mentions` link, origin `inline`, `dst` a path, `src` as an ID mention's. `path` = before the first `#`, cut at the first `?` (`""` for `#h`); `anchor` = after it, absent when empty; both as pulldown-cmark gives them, never percent-decoded; no path, no anchor → nothing. `span` = the destination's bytes (`<…>`, title excluded; `?query`, `#anchor` included), re-scanned after the `](` or `]:`: blanks include VT, FF; a bare destination ends at a byte ≤ 0x20 (DEL included); unlocatable → not recorded. Never: images and links inside them, autolinks, HTML, reference uses, wiki links, code, front-matter. Order: declared links, then the body's mentions and file links by span start. The store keeps `dst` as written (`dst_path`).

**Config.** `[paths] link_base` (optional): a root-relative directory under the roots' path rules (`"/docs"`, `"../x"`, `"a/./b"`, `""`, a non-string → `specengine.toml:<line>: message`, cannot check). The check's only: outside the index fingerprint; existence unchecked; no default (ADR-0008).

**Resolution.** Checked: a path ending exactly in `.md` after percent-decoding (a malformed `%` stays; a non-UTF-8 result → as written), or an empty path with an anchor (the linking file). Candidates, normalised (`..` pops; popping above the root leaves it): `/`-led → one, from the root; else C1 = the linking file's directory + path, then, only if C1 names no walked document, C2 = `link_base` + path (Q-G). The first naming a walked document (any class, tier, status) resolves. Else, if a candidate lies in the walk scope (`WalkScope::in_walk_scope`; a symlinked `.md` there is not walked: a warning, accepted) → `link-dangling`; else nothing. From `docs/records/REQ/REQ-001.md`, `link_base = "docs"`:

| Written | Outcome |
|---|---|
| `REQ-002.md`, `../ASM/ASM-01.md#h` | resolved (C1), `#h` checked there |
| `spec/cli.md#CMD-SYNC` | C1 missing, C2 `docs/spec/cli.md` resolves |
| `/docs/spec/cli.md` | `docs/spec/cli.md`, no base |
| `spec/gone.md`, `a%20b.md` | `link-dangling` |
| `#h` | checked in the linking file |
| `../../../README.md` | `README.md`, under no root: nothing |
| `LICENSE`, `../`, `x.rs`, `x.MD` | recorded, never checked |
| `https://h/x.md`, `//h/x.md`, `mailto:a` | not recorded |

**Anchors.** A resolved link's anchor, decoded, is one of the target's `anchors` or section IDs — `canon-anchor`'s predicate (`has_anchor`), exact — else `link-anchor`. A target without a parse or not UTF-8: unchecked. Slugs differing from GitHub's (model README "Anchors"): a rare false warning, accepted.

**Findings**: warnings (`#control`, baselineable) from live sources (neither `class: generated` nor Tier 3), at the destination's line, subject the text under its span (`<x y.md>` → `x y.md`), in any input order the same:

    warning  docs/records/REQ/REQ-001.md:14: link-dangling: `spec/gone.md` names no walked document (tried `docs/records/REQ/spec/gone.md`, `docs/spec/gone.md`)
    warning  design/notes.md:8: link-anchor: `sections.md#nope`: `design/sections.md` has no anchor or section `#nope`

**Accepted deviations and limits.**

- `tried` lists a C2 equal to C1 once, never a candidate leaving the root (C1 included). `link-anchor` shows the decoded anchor, its subject as written.
- An existing but excluded C1 with a missing in-scope C2 → a false `link-dangling`.
- Locality is judged on the trimmed destination, `path` kept as given: `[a](<x.md >)` records `"x.md "`, never checked (the census checks it).
- A `>` on a 4+-indented continuation line is text to pulldown-cmark, skipped by the re-scan: `[a](\n    >)` records nothing; `[a](\n    >x.md)` records path `>x.md` with span `x.md`, so a warning's subject (`x.md`) and candidate (`docs/>x.md`) disagree.

## Open owner questions

Working answer (the code) → what the other answer triggers.

- Q-E ADR-0026 supersedes the old layout decision, restating its unchanged part → the old one back to `accepted`, ADR-0026 without `supersedes`, only the new rule.
- Q-F `id-scope` is an error → a warning: the severity where the engine pushes it (`CHECK_CODES` has no severity column); the `id-scope` test of `check_scopes.rs` flips.
- Q-G `link_base` is a fallback after file-relative resolution → exclusive: pass B's resolution order only.
- Q-I criteria lose their front-matter for now, `links.verifies` becoming an inline mention in the section body → heading attributes now: a parser change, with core Q3.
- An escaped `parent:` loses its qualifiers and section → `ParentRef` carries `scope` and `project`: a model and `INDEX_FORMAT` change.
