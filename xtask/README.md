---
class: canon
tier: 1
scope: [xtask]
owner: owner
reviewed: 2026-09-28
---

# xtask — enforcement of the documentation convention

Enforces §11 of `docs/canon/documentation-system.md` in this repository until SpecEngine's own `spec check` replaces it (ADR-0013, ADR-0022). No dependencies — std only, builds in seconds.

## Commands

| Command | What it does | Exit |
|---|---|---|
| `cargo xtask docs check` | the six §11 checks over all documents | 0 — clean, 1 — errors, 2 — failed to run |
| `cargo xtask docs index --write` | rebuilds `docs/index.md` from front-matter | 0 |
| `cargo xtask docs index` | prints the index to stdout, leaves the file alone | 0 |
| `cargo xtask docs budget` | sizes against caps and the worst-case working set W | 0 |

## What counts as a document

Every `*.md` in the repository, except the directories `.git`, `.claude` (role prompts), `.github`, `target*`, `node_modules`, `dist`, and files starting with `_` (templates).

## Modules

| Module | Responsible for |
|---|---|
| `src/docs/mod.rs` | repository walk, document model, headings, anchors (GitHub slug, `{#id}`, `<a id>`), archive flag |
| `src/docs/frontmatter.rs` | the YAML subset: scalars, `[a, b]`, block lists; anything else is an error with a line number |
| `src/docs/check.rs` | schema per class, budgets, `canon:`, ADR references, index against front-matter |
| `src/docs/index.rs` | deterministic index rendering: canon, decisions, specs, archive |
| `src/docs/budget.rs` | §4 caps and the W report |

## Rules for changes

- A new check comes only from §11 or an ADR. A noisy check is worse than none: one line per finding and one summary line.
- Caps are constants in `budget.rs` with their source in a comment; they are never raised (§4).
- A new generator is registered in `GENERATORS` (`check.rs`): its output is then checked for drift the same way as the index.
