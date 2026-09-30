---
class: canon
tier: 1
scope: [xtask]
owner: owner
reviewed: 2026-10-01
---

# xtask — enforcement of the documentation convention

Enforces §11 of `docs/canon/documentation-system.md` in this repository until SpecEngine's own `spec check` replaces it (ADR-0013, ADR-0022). No dependencies — std only, builds in seconds.

`spec check` (`docs/canon/spec-check.md`, `docs/canon/spec-check-graph.md`) covers all six §11 checks plus the ADR-0009 ID checks and graph warnings, at parity (`crates/specengine-store/tests/check_parity.rs`, its index render byte for byte against `docs index`). CLI pass 2b (`docs/canon/spec-check-cli.md`) switches hook and CI to `spec` and retires this crate; until then it is the gate.

## Commands

| Command | What it does | Exit |
|---|---|---|
| `cargo xtask docs check` | the six §11 checks over all documents | 0 — clean, 1 — errors, 2 — failed to run |
| `cargo xtask docs index --write` | rebuilds `docs/index.md` from front-matter | 0 |
| `cargo xtask docs index` | prints the index to stdout, leaves the file alone | 0 |
| `cargo xtask docs budget` | sizes against caps and the worst-case working set W | 0 |
| `… docs <command> --root DIR` | the same over `DIR` instead of this repository (the parity test's differential run) | as the command; `--root` without `DIR`: usage, 2 |

## What counts as a document

Every `*.md` in the repository, except the directories `.git`, `.claude` (role prompts), `fixtures` (test corpora with foreign conventions), `.github`, `target*`, `node_modules`, `dist` (`SKIP_DIRS` in `src/docs/mod.rs`, matched by name at any depth), and files starting with `_` (templates).

## Modules

| Module | Responsible for |
|---|---|
| `src/docs/mod.rs` | repository walk, document model, headings, anchors (GitHub slug, `{#id}`, `<a id>`), archive flag |
| `src/docs/frontmatter.rs` | the YAML subset: scalars, `[a, b]`, block lists; anything else is an error with a line number |
| `src/docs/check.rs` | schema per class, budgets, `canon:`, ADR references, index against front-matter |
| `src/docs/index.rs` | deterministic index rendering: canon, decisions, specs, archive (a Tier 3 line is only link and status, ADR-0028) |
| `src/docs/budget.rs` | §4 caps and the W report |

## Rules for changes

- A new check comes only from §11 or an ADR. A noisy check is worse than none: one line per finding and one summary line.
- Caps are constants in `budget.rs` with their source in a comment; they are never raised (§4).
- A new generator is registered in `GENERATORS` (`check.rs`): its output is then checked for drift the same way as the index. For `spec check` parity it also goes into `[[generators]]` of the parity config (`check_parity.rs`).
