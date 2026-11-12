---
class: generated
generator: cargo run -q -p specengine-cli -- export index
source: front-matter of the repository's documents
---

# Documentation index

<!-- Built by `cargo run -q -p specengine-cli -- export index`. Manual edits are overwritten on rebuild, and `cargo run -q -p specengine-cli -- check` rejects them. -->

Reading protocol (§9): this index, then at most three documents. Needing a third step means the index is wrong: fix it rather than reading further.

## Canon

- [CLAUDE.md](../CLAUDE.md) SpecEngine · root · tier 0
- [README.md](../README.md) SpecEngine · root · tier 2
- [docs/README.md](README.md) Documentation: how the convention is applied · docs · tier 1
- [scripts/deploy/README.md](../scripts/deploy/README.md) scripts/deploy -- a hub on one host · scripts/deploy · tier 1
- [ui/README.md](../ui/README.md) ui -- the web UI · ui · tier 1

## Specs

- [docs/features/bevy-dump.md](features/bevy-dump.md) bevy-dump: the project's schedule dump annotates symbols (layer B) · crates/specengine-code, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-eval, crates/specengine-mcp, fixtures · draft
- [docs/features/delivery-analytics.md](features/delivery-analytics.md) delivery-analytics: what was delivered, what is overdue and why, per release and per department (RM-0107 a) · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, fixtures, ui · draft
- [docs/features/design-sweep.md](features/design-sweep.md) design-sweep: the legacy names gone, the guards closed, Inbox, Questions and the Spec tree on the primitives (RM-0099 a) · ui · draft
- [docs/features/direction-catalog.md](features/direction-catalog.md) direction-catalog: a direction as a table of its documents, with version, state, open items, tasks and last change (RM-0106) · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, fixtures, ui · draft
- [docs/features/intake-similar.md](features/intake-similar.md) intake-similar: a create lists what it may repeat, and an exact twin waits to be named (RM-0059) · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-eval, plugin, scripts, fixtures, ui · draft
- [docs/features/node-history.md](features/node-history.md) node-history: a node's changes, who made them and when, on the CLI, over MCP and in the document view (RM-0035) · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, plugin, scripts, fixtures, ui · draft
- [docs/features/pack.md](features/pack.md) pack: spec init --pack installs a preset, and a second, non-game preset (RM-0061) · crates/specengine-cli, scripts, fixtures · draft
- [docs/features/upstream-signal.md](features/upstream-signal.md) upstream-signal: a subtask sees what changed above it, on the CLI, over MCP and on the Board (RM-0105) · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, plugin, scripts, fixtures, ui · draft
- [docs/features/zone-index.md](features/zone-index.md) zone-index: canon under the index entry's unlisted globs takes no line, and canon retires by status (RM-0055) · crates/specengine-core, crates/specengine-store, crates/specengine-cli, docs · draft
- [docs/specs/specengine-platform/04-prior-art-and-stack.md](specs/specengine-platform/04-prior-art-and-stack.md) 04. Prior art, MCP, Claude Code, the stack · specengine · in-progress
- [docs/specs/specengine-platform/05-architecture.md](specs/specengine-platform/05-architecture.md) 05. Target architecture of SpecEngine · specengine · in-progress
- [docs/specs/specengine-platform/06-workflows.md](specs/specengine-platform/06-workflows.md) 06. Workflows · specengine · in-progress
- [docs/specs/specengine-platform/07-interfaces.md](specs/specengine-platform/07-interfaces.md) 07. Interfaces: MCP, CLI, HTTP, hooks, config · specengine · in-progress
- [docs/specs/specengine-platform/08-roadmap.md](specs/specengine-platform/08-roadmap.md) 08. Roadmap: phases, import, criteria, risks · specengine · in-progress
- [docs/specs/specengine-platform/README.md](specs/specengine-platform/README.md) SpecEngine — research and platform concept · specengine · in-progress

## Shards

- [docs/index-archive.md](index-archive.md) Archive — Tier 3, by id only
- [docs/index-canon-spec.md](index-canon-spec.md) `docs/canon/spec-*.md`
- [docs/index-canon-proposal.md](index-canon-proposal.md) `docs/canon/proposal-*.md`
- [docs/index-canon-hub.md](index-canon-hub.md) `docs/canon/hub*.md`
- [docs/index-canon-card.md](index-canon-card.md) `docs/canon/card*.md`
- [docs/index-canon.md](index-canon.md) `docs/canon/*.md`
- [docs/index-decisions.md](index-decisions.md) `docs/decisions/*.md`
- [docs/index-crates.md](index-crates.md) `crates/*/README.md`
- [docs/index-roadmap.md](index-roadmap.md) `docs/roadmap/*.md`
