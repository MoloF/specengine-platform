---
class: canon
tier: 1
scope: [docs]
owner: owner
reviewed: 2026-10-01
---

# Documentation: how the convention is applied

The norm is `docs/canon/documentation-system.md` (ADR-0022). This document describes how it is applied in this repository. SpecEngine applies the same rules to every project it manages. All content is in English (ADR-0024).

## Reading protocol

`CLAUDE.md` → `docs/index.md` → at most three documents. Needing a third step means the index or the canon is incomplete: fix it rather than reading further. The archive (Tier 3) is read by id only. Do not open a file longer than two hundred lines whole: headings first (`grep '^#'`), then the section you need.

## Classes and where they live

| Class | Answers | Where | Lifecycle |
|---|---|---|---|
| canon | how it works now | `CLAUDE.md`, `README.md`, `*/README.md`, `docs/canon/` | rewritten in place |
| decision | why, and when | `docs/decisions/ADR-NNNN.md` | never edited, only superseded |
| spec | what we are doing now | `docs/features/<slug>.md`, `docs/specs/` | consumable: compacted after shipping |
| generated | what exists | `docs/index.md` | written by the generator only |

Misclassification is the main source of rot. A spec in canon position lies within weeks. A canon written as a decision log forces everyone to replay history.

## Tiers

- **Tier 0** — `CLAUDE.md`, always read.
- **Tier 1** — a subtree's `README.md`, read when working inside it: `docs/README.md`, `crates/<crate>/README.md` (one per crate, created with the crate), later `ui/README.md`. Canon lives next to the code it describes.
- **Tier 2** — the index and documents read on an explicit question: `docs/canon/`, the root `README.md`, decisions, live specs.
- **Tier 3** — archive by status: specs `shipped`/`abandoned`, decisions `superseded-by`/`rejected`. **Front-matter, not folder location**, excludes a document. Its index line keeps only the id link and the status (ADR-0028).

## Budgets (bytes of the whole file, front-matter included)

| Slot | Cap | Source |
|---|---|---|
| Tier 0 | 16 384 | §4 |
| Tier 1 | 10 240 per subtree | §4 |
| Index | 10 240 | §4 |
| Decision | 1 536 | §4 |
| Tier 2 canon | 12 288 | repository calibration (§4 step 2: recompute from the average of five documents actually opened after the first working week; the cap may only go down) |
| Spec | none; after shipping — intent + summary ≤ 3 KB | §6 |

Overflow moves detail down a tier; the cap is **never raised**. The summary line of `spec check` shows the worst working set W (`docs/canon/spec-check.md` "Output").

## Front-matter contract

```yaml
# canon
class: canon
tier: 0 | 1 | 2
scope: [subtree]
owner: owner
reviewed: YYYY-MM-DD
# decision — template docs/decisions/_template.md
class: decision
id: ADR-NNNN
title: what was decided
status: accepted | rejected | superseded-by ADR-NNNN
date: YYYY-MM-DD
scope: [subtree]
canon: path#anchor       # required when accepted
supersedes: [ADR-…]      # optional
ref: source              # optional
# spec — template docs/features/_template.md
class: spec
status: draft | in-progress | shipped | abandoned
scope: [subtree]
ref: source              # optional
shipped: YYYY-MM-DD      # required when shipped
adrs: [ADR-…]            # optional
# generated
class: generated
generator: command
source: what it is built from
```

Extra keys are errors. Files starting with `_` are templates, not documents. A `canon:` anchor is a GitHub heading slug, `{#id}` in a heading, `<a id="…">` or `<a name="…">`.

## Promotion rule

An accepted decision produces a canon diff **in the same change**, and its `canon:` points at the changed section. Steps:

1. copy `docs/decisions/_template.md` to the next free `ADR-NNNN.md`;
2. change the canon section (`CLAUDE.md`, a Tier 1 README or `docs/canon/*`) and point `canon:` at it;
3. if the decision replaces an older one — the old one gets `status: superseded-by ADR-NNNN`, the new one `supersedes: [...]`;
4. `cargo run -q -p specengine-cli -- export index`, then `cargo run -q -p specengine-cli -- check`.

Self-test: can you answer "how does X work now" without opening a single ADR? If not, the canon is incomplete.

## Spec lifecycle

`draft` → `in-progress` → `shipped` (+ `shipped: date`) or `abandoned`. On shipping, a spec is compacted to intent, criteria and a summary ≤ 3 KB. Plans, implementation notes and review transcripts are not kept: they are exhaust, 60–70 % of the bytes of a feature. Whatever became true about the system moves into the canon.

## Generated

`docs/index.md` is written only by `cargo run -q -p specengine-cli -- export index`, the generator the root `specengine.toml` registers. A manual edit fails the check (`index-drift`). Everything derivable from code is generated, not written.

## Enforcement

`spec check` (`docs/canon/spec-check*.md`, CLI: `docs/canon/spec-check-cli.md`) is the only check: the six of §11 (budgets, front-matter schema, `canon:` resolves, `superseded-by`/`supersedes`/`adrs` targets exist, index and generated documents have not drifted), IDs and scopes, graph and file-link warnings, a debt baseline. The root `specengine.toml` configures it.

- **Walk**: roots `CLAUDE.md`, `README.md`, `crates`, `docs`; `_*.md` and every `fixtures`, `target`, `target.noindex`, `node_modules`, `dist` directory excluded. A new top-level directory or `.md` file (`ui/`, `plugin/`, `AGENTS.md`) stays unwalked until listed in `roots`: the task creating it adds it in the same change.
- **Roles** run `cargo run -q -p specengine-cli -- export index && cargo run -q -p specengine-cli -- check` before handing in (`CLAUDE.md`, "Process").
- **Pre-commit hook** (`scripts/hooks-install.sh` enables `.githooks/`): when the staged names (renames split, `--diff-filter=ACDMT`) include a `.md` file or the top-level `specengine.toml` or `.spec-debt.toml`, it runs `cargo run -q -p specengine-cli -- check --staged --root .`. Fail closed: any non-zero exit (errors, cannot check, a failed build, no toolchain) refuses the commit; only `--no-verify` skips. The checker is built from the working tree. Only convention form errors block (ADR-0022); content never does (ADR-0006).
- **Merges**: a clean `git merge` runs `pre-merge-commit`, not `pre-commit`; merged documents are judged by CI alone.
- **CI** `.github/workflows/docs.yml`: `cargo run --locked -q -p specengine-cli -- check --root .` on `HEAD`, its only step.

## Compaction

Quarterly, or when the index nears its cap: mark superseded decisions, absorb settled decisions into the canon and trim the record to the "why", drop old shipped specs, regenerate the index. Skipping compaction costs not storage but truth.
