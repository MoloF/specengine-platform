---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-model, crates/specengine-store]
ref: 08-roadmap.md Phase 1, spec check increment 2 part 2, pass A of 2 (owner answer Q-D, 2026-09-29)
shipped: 2026-09-30
adrs: [ADR-0026]
---

# spec check, increment 2 part 2, pass A: feature scopes

## Why

Citations of feature criteria were never checked: every `slug/` reference resolved to `Skipped`, and feature-scoped prefixes were exempt from `id-taken`. Both fixtures hid a dead citation: spec-a cited `stamina-tuning/AC-07` while `AC-07` was a record file, spec-b `dry-run/CRIT-01` likewise. The owner settled the layout (Q-D: ADR-0026, `docs/canon/architecture.md#layout`); this pass turns dead criterion citations and misplaced criterion definitions into findings, without noise. Pass B, `spec-check-links`, follows (Q-H). The rules now: `docs/canon/spec-check-links.md`.

## Acceptance criteria

Tests in `crates/specengine-core/tests/` unless named; each names the mutation that turns it red.

- [x] AC-01 Documents: ADR-0026 accepted, `supersedes` the old layout decision, `canon: docs/canon/architecture.md#layout`, ≤ 1 536 B; the old one `status: superseded-by ADR-0026`; `cargo xtask docs check` green; both `docs/canon/spec-check*.md` and the new `docs/canon/spec-check-links.md` ≤ 12 288 B; worst W (`cargo xtask docs budget`) ≤ 117 863 B at shipping; the scope rules answerable from `docs/canon/` alone. Red: the ADR over its cap; `canon:` at a missing anchor.
- [x] AC-02 Feature documents (new `check_scopes.rs`): with `features = "specs/feat"` and a feature-scoped `AC`, `{#AC-01}` in `specs/feat/a.md` → nothing; `{#AC-02}`, `{#AC-03}`, `{#AC-04}` in `specs/feat/sub/b.md`, `specs/feat/Upper.md`, `docs/features/c.md` → one `id-scope` each; `a/AC-01` resolves, `c/AC-04` dangles. Red: nested files accepted; the default directory hard-coded.
- [x] AC-03 Scoped resolution (`check_scopes.rs`): `feat/AC-01` with `{#AC-01}` in `docs/features/feat.md` → nothing; `feat.md` absent, or without AC-01 → inline `mention-dangling`, front-matter `ref-dangling`, the message naming `docs/features/feat.md`; `feat/AC-01#AC-02` needs AC-02 in `feat.md`; `feat/R-97` does not resolve to an R-97 defined elsewhere; `other:feat/AC-98` → nothing. Red: `slug/` still `Skipped`; `feat/AC-01` resolving to another file's AC-01.
- [x] AC-04 Bare IDs (`check_scopes.rs`): bare `AC-01` inside `feat.md` → its own section; in `docs/spec/x.md` while `feat.md` and `other.md` define it → `mention-dangling`, the reason naming `feat/AC-01` then `other/AC-01`; in front-matter → `ref-dangling`; defined nowhere scoped → the "no feature document" reason. Red: a global lookup for feature-scoped IDs.
- [x] AC-05 `id-scope` (`check_scopes.rs`): `docs/records/AC/AC-07.md` with `id: AC-07`; `{#AC-01}` in `docs/spec/x.md`; `{#AC-01}` in `docs/features/sub/y.md`; `id: AC-02` in `docs/features/feat.md` → one error each at the definition line, subject the ID, no `id-taken` or `file-name` beside it; a misplaced `id: AC-7` keeps its `id-width`. Red: warning severity; the defect reported twice.
- [x] AC-06 Uniqueness (`check_ids.rs`): two features each with `{#AC-01}` → nothing; one feature defining AC-01 twice → `duplicate-id` only; duplicate project-scoped IDs → `id-taken` as before. Red: a global `id-taken` for feature-scoped IDs.
- [x] AC-07 Graph (`check_graph.rs`): a cycle through `depends_on: [feat/AC-01]` → exactly one `depends-cycle`. Red: scoped edges ignored.
- [x] AC-08 Output (`check_output.rs`): reversed input → the same lines and JSON; `CHECK_CODES` has 27 entries, `id-scope` among them; a baseline entry (`id-scope`, path, ID) turns the finding into debt and `enforce` into `clean`. Red: `HashMap` order in the reasons.
- [x] AC-09 Fixtures (`genre.rs`, `check_genre.rs`, `records.rs`): both corpora match their new `expected.json`; blocking sets as pinned (spec-a one `frontmatter-type`; spec-b's five pairs); warnings as pinned (spec-a one `depends-cycle`, spec-b one `mention-dangling`, `docs/spec/cli.md:25`); no `id-scope`, no new dangling; spec-a record prefixes `A, DEC, Q, R, TERM`; documents spec-a 13 (store `check_verdict.rs`, eval `check_cli.rs`), spec-b 11. Red: keeping `docs/records/AC/AC-07.md`; the bare `CRIT-01` left in `cli.md`.
- [x] AC-10 Genre (`check_genre.rs`): the new sources are scanned, no prefix, slug, `features/`, `records/` or `docs/` literal. Red: a special case for `"AC"`.
- [x] AC-11 Format (store `format.rs`): `INDEX_FORMAT` 4; the history ends with exactly one new `4 <hash>` line and no earlier `4`; part 1's "keeps the format" test replaced by one pinning 4. Red: the fixtures edited without a new history line.
- [x] AC-12 Parity (store `check_parity.rs`): `enforce`, no baseline → `clean`, 0 debt; among part 1's codes plus `id-scope`, exactly the one pinned `mention-dangling` of `docs/canon/spec-check.md` line 50; every seed blocks with its code, "accepted ADR without canon:" on ADR-0026. Red: the superseded decision left in the `adrs:` of `docs/specs/specengine-platform/README.md` (one `ref-superseded`).
- [x] AC-13 Updated tests: `check_refs.rs` front-matter `stamina-tuning/AC-07` without its feature → `ref-dangling`; `check_mentions.rs` `feat/AC-99` and `feat/R-97` → `mention-dangling`, `other:feat/AC-98` → nothing; the public `Resolver` test uses the new signature. Red: a `slug/` reference skipped.
- [x] AC-14 Read-only and dependencies: `git status --porcelain` unchanged by the tests; no new workspace key (eval `build_graph.rs`); full `cargo nextest run`, `cargo clippy`, `cargo fmt --check` green.

## Implementation

Two iterations, review accepted in both; the full workspace run: 630 passed, 15 skipped. (1) Feature documents via `grammar::is_slug`, scoped `slug/ID` and bare feature-scoped resolution, `id-scope`, graph rules from the citing file, the public `Resolver` change, `INDEX_FORMAT` 4, `CHECK_CODES` 27, the fixtures moved (spec-a now 13 documents, spec-b 11). (2) `parent:` keeps its qualifiers and `#section` through a re-parse of the verbatim bytes under its span (no model change); fewer allocations in `holders_in`, `in_section`.

| Module | What it does |
|---|---|
| model `grammar.rs` | `is_slug`, the `slug` rule shared with the lexer's qualifier look-back |
| core `check/resolve.rs` | `Resolver::new(.., &Paths)`; places `Anywhere`, `Feature`, `Own`; `feature_slug`; `from` on `resolve`, `resolve_mention`, `holders_of` (owned `Vec`); the five reasons; `parent_reference`; `written` rebuilds qualifiers |
| core `check/engine.rs` | `id-scope`, suppressing `id-taken` and `file-name` for its definition; references resolved from the citing file |
| core `check/graph.rs` | mentions, `depends-cycle`, `ref-superseded` resolved from the citing file |
| core `check/mod.rs`, store `lib.rs` | `CHECK_CODES` 27 (`id-scope` last); `INDEX_FORMAT` 4, store code unchanged |
| fixtures | spec-a `AC-07` into `features/stamina-tuning.md`; spec-b `CRIT-01` into `features/dry-run.md`, `cli.md:25` → `dry-run/CRIT-01`; both `expected.json` |
| tests | core `check_scopes.rs` new, `check_{ids,refs,mentions,graph,output,genre}.rs`, `records.rs`; model `grammar.rs`; store `check_{parity,verdict}.rs`, `format.rs`, `format_history.txt`; eval `check_cli.rs` |

Deviations from the draft, deliberate:

- A reference without a span shows as `project:slug/ID#Y` in messages (text only).
- `parent:` now has its `#Y` checked and `parent: other:X` is `Skipped`, as `docs/canon/spec-check.md` "Rules" says of every front-matter reference. Limit: an escaped value has no span and loses both (`"feat\/AC-01"` outside a feature: a false `ref-dangling`); the fix, `ParentRef` with `scope` and `project`, is a model and `INDEX_FORMAT` change: an open owner question.
- Q-F said a warning would change "its row in `CHECK_CODES`": that table has no severity; the engine fixes it where it pushes the finding.
- S2 for definitions cannot occur: the parser never defines a legacy-prefix ID; only references are scoped through `aliases_from`.
- `check_ids.rs`' record-count threshold is 8 (spec-a lost `AC-07.md`).
