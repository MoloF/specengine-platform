---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store]
ref: 08-roadmap.md Phase 1, spec check increment 2 part 2, pass B of 2; 08 §4.3 (a)
shipped: 2026-09-30
adrs: []
---

# spec check, pass B: file links

## Why

Markdown file links were not recorded, so a renamed or deleted document left dead links nobody saw, here and in every managed project. A naive check is noise: 57 of one pilot's 58 "broken" links point to existing files, written relative to the docs root (08 §4.3 (a)). This pass records local destinations as weak `mentions` with a path destination, resolves them file-relative with a per-corpus fallback base, and reports dead ones as warnings. No ADR: ADR-0008 (the base is config), ADR-0012 (warnings), ADR-0013 (a new rule, nothing to port). The rules now live in `docs/canon/spec-check-links.md` "File links".

## Acceptance criteria

Tests in `crates/specengine-core/tests/` unless named.

- [x] AC-01 Parser (`file_links.rs`): one `mentions` Path link each, span exactly the destination, for inline links (`<…>`, a title, `x\_y.md`, `?plain=1`, `#h` alone, nested brackets, a code span holding `](` in the text, text over two lines, a table cell, a heading, inside an ID section), a definition used or not (one link), a definition's destination on the next line of a CRLF file. None for external URLs, `//h`, `mailto:`, `[a]()`, `[a](<>)`, `[a](#)`, images, autolinks, HTML, code, `[[x]]`. Red: an external URL recorded; the whole link range as the span.
- [x] AC-02 A file link gives no `mention-dangling`, no `ref-superseded` (`check_mentions.rs`); eval corpus-mini `references.inline` stays 4 (`parse_cli.rs`). Red: Path links counted (7).
- [x] AC-03 Store: spec-a `dst_path` rows `stamina.md`, `sprint.md`; `file(path)` equals the core's parse; incremental equals fresh after a link edit; `INDEX_FORMAT` 5 with exactly one new `5 <hash>` history line (`format.rs`). Red: fixtures changed without a new history line.
- [x] AC-04 `link_base = "docs"` and `"docs/"` accepted; `"/docs"`, `"../x"`, `"a/./b"`, `""`, `1` → `specengine.toml:<line>: message`, cannot check (core and store `check_config.rs`); editing only `link_base` → `parsed: 0`. Red: `link_base` in the fingerprint.
- [x] AC-05 Resolution (`check_links.rs`): every row of the canon's table; C1 wins when both are walked; C2 only after C1; no base → `spec/cli.md` from a nested file dangles; anchors by slug, attr, html, section ID; unknown → `link-anchor`; a percent-encoded Cyrillic anchor matches its slug. Red: the base tried first; no percent-decoding.
- [x] AC-06 Nothing for `LICENSE`, `docs/decisions/`, `x.rs`, targets outside every root, excluded, below a `.`-named directory or above the root, nor from generated or Tier 3 sources; a failed front-matter source is checked; the store walker uses core's matcher (`walk.rs`). Red: `LICENSE` flagged; an excluded target flagged.
- [x] AC-07 Warning severity; subject as written; at the destination's line; `enforce` stays `clean`; baselineable; reversed input → same lines and JSON; `CHECK_CODES` 29, ending `link-dangling`, `link-anchor` (`check_links.rs`, `check_output.rs`). Red: error severity.
- [x] AC-08 Eval `links_census.rs`: on corpus-mini (`roots = ["design"]`) the census's `broken_links` equal the `link-dangling` set — exactly `design/sections.md:10 missing.md`; no `link-anchor`. Red: resolving from the root instead of the linking file.
- [x] AC-09 spec-b gains `link_base = "docs"` and `spec/cli.md#CMD-SYNC` in `docs/records/REQ/REQ-001.md`, resolving only through the base; both `expected.json` list the links; 0 link findings, other pins unchanged (`genre.rs`, `check_genre.rs`, store `check_verdict.rs`). Red: `link_base` removed from spec-b.
- [x] AC-10 Parity (store `check_parity.rs`, no `link_base`): 0 link findings, the one pinned `mention-dangling` at `docs/canon/spec-check.md:50`; `README.md` records 9 file links (`docs/decisions/`, `LICENSE` unchecked), the platform spec's `README.md` 6 sibling links, all walked. Red: C1 joined to the linking file's path instead of its directory (the six sibling links dangle). Non-`.md` targets checked cannot turn this red here (they are never in scope): `check_links.rs::unchecked_and_out_of_scope_targets_give_nothing` covers `REQ-002.md/#nope`, `gone.md/`, `gone.md.`.
- [x] AC-11 Docs check green; `docs/canon/spec-check.md`, `docs/canon/spec-check-links.md` ≤ 12 288 B; the link rules answerable from `docs/canon/`; worst W ≤ 117 789 B. Red: a canon document over its cap.
- [x] AC-12 `check_genre.rs` scans `links.rs` (no `docs` literal, no default base); no new dependency (eval `build_graph.rs`); tests leave `git status` clean; full nextest, clippy, fmt green. Red: a hard-coded `link_base` default.

## Implementation

Four iterations, review accepted in each; the full workspace run: 692 passed, 15 skipped. (1) Parser, `link_base`, the check, `INDEX_FORMAT` 5, `CHECK_CODES` 29, eval counter. (2) Review: globs compiled once in a public `WalkScope`, one `has_anchor`, one `.md` constant, core exports `is_under`, `is_clean_relative`. (3) Blanks before a destination include VT/FF (a debug-build panic removed); a bare destination ends at a byte ≤ 0x20; `DOCUMENT_EXTENSION` public, used by the store walker. (4, owner-approved) Both `debug_assert!`s removed: an unlocatable destination is not recorded in any build.

| Module | What it does |
|---|---|
| core `markdown.rs` | local inline links and reference definitions; the destination span by re-scan |
| core `lib.rs` | file links into `ParsedFile.links`, by span start with ID mentions; crate-root exports |
| core `walk_scope.rs`, `glob.rs` (from the store) | `WalkScope`, `DOCUMENT_EXTENSION`, `is_under`, `is_clean_relative`; census globs |
| core `paths_toml.rs` | `[paths] link_base`; `Paths::walk_scope` |
| core `check/links.rs` | `link-dangling`, `link-anchor` |
| core `check/{engine,graph,mod,resolve}.rs` | shared `has_anchor`, `live_sources`, `warning`; `CHECK_CODES` 29 |
| model `link.rs`, `reference.rs` | `Path` also a Markdown destination (docs only) |
| store `source.rs`, `write.rs`, `lib.rs` | the walker on core's `WalkScope`; `INDEX_FORMAT` 5 |
| eval `parse.rs` | `references.inline`: ID references only |

Deviations and limits, all in the canon's "Accepted deviations and limits": an existing but excluded C1 with a missing in-scope base candidate warns falsely (accepted in the draft); a C2 equal to C1 is listed once and no root-leaving candidate is listed; a non-UTF-8 target's anchors are unchecked; `link-anchor` shows the decoded anchor; locality is judged trimmed while `path` is kept as given (`[a](<x.md >)` records `"x.md "`, a divergence from the census); a `>` on a 4+-indented continuation line misplaces the span (subject and candidate disagree).
