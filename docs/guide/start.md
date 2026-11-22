---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-22
summarises: [docs/canon/presets.md#the-command, docs/canon/architecture.md#layout, docs/canon/architecture.md#storage, docs/canon/documentation-system.md#3-tiers-and-read-triggers, docs/canon/documentation-system.md#12-adoption-order]
---

# Adopting SpecEngine

SpecEngine keeps a project's specification in the project's own git repository, next to the code, and gives people and agents one fixed way to find it, change it and cite it. This page covers the first day: what to install, what `spec init` writes, and where each kind of document goes, so nobody has to keep it in their head.

## What it solves

Without a fixed layout, every person and every agent invents a place for a rule, a decision or a task. Six months later, answering "how does stamina work" means reading the whole history. SpecEngine fixes the places in one config file, `specengine.toml`, and `spec check` enforces them. Reads then return the one node you asked for, so the cost of a task (W, the bytes read to do it) stays flat as the project grows.

## When to use it

- **On day one** of a new project, before the first design document exists. Front-matter and classes cost nothing now and a migration later.
- **When you bring an existing documentation set under control.** Sort each file into one of the four classes (`guide:records#GD-RECORDS-FOUR-CLASSES`), then let the check hold the line.
- **For any team shape.** A game studio with narrative, art, sound and programming departments, or a software team with product, UX, engineering and QA. The departments are data in your config, not SpecEngine's code.

## How

### CLI

Install `spec` and the MCP server from one commit, then initialise the project at its repository's top:

```sh
cargo install --locked --force --path <clone>/crates/specengine-cli
cargo install --locked --force --path <clone>/crates/specengine-mcp
cd lamp-game
spec init --pack <clone>/scripts/presets/gamedev.toml
```

```text
created specengine.toml with slug lamp-game from <clone>/scripts/presets/gamedev.toml
```

`spec init` writes exactly one file, `specengine.toml`, and never overwrites one. `--pack` copies a preset's card flows and departments into it; two presets ship, `gamedev.toml` and `software.toml`. Then add your own tables (an example is under "Where it lives") and build the index:

```text
$ spec index
indexed lamp-game: walked 13, parsed 13, unchanged 0, removed 0, unreadable 0
$ spec export index
wrote docs/index.md: 2210 bytes
$ spec check
spec check [enforce]: 14 documents, 0 errors, 0 warnings, 0 debt, 0 expired, 0 stale, worst W 21480 B — clean
```

The last figure, worst W, is the most a task can cost to read in your corpus; it should stay flat as the project grows.

### MCP

Install the Claude Code plugin per project. It brings the stdio MCP server, the skills, the roles and the write gate:

```sh
claude plugin marketplace add <clone>/plugin
claude plugin install specengine@specengine --scope local
```

An agent reads with `get_tree`, `get_node`, `search` and `get_context_bundle`, and asks "where does X go" with `get_guide`. It never writes a spec file (`guide:proposals#GD-PROPOSALS-ONE-DOOR`).

### Screen

`spec serve` in the project starts the local daemon and prints the address of the web UI, `http://127.0.0.1:7777/` by default (install `specengine-http` from the same commit). Sections include Inbox, Board, Spec tree, Documents, Flow, Roadmap, Health and Questions. Each department sees its own set of sections (`guide:cards#GD-CARDS-DEPARTMENT-IS-A-LENS`).

## Where it lives

A typical `specengine.toml` after the first day. The preset's `[[card.flow]]` and `[[workspace]]` tables are left out here:

```toml
[project]
slug = "lamp-game"

[paths]                       # each role defaults to docs/<role>
roots = ["CLAUDE.md", "docs"]
tier0 = "CLAUDE.md"
index = "docs/index.md"

[ids]
DOM  = { kind = "domain",      shape = "name" }
MEC  = { kind = "mechanic",    shape = "name" }
R    = { kind = "requirement", width = 3 }
Q    = { kind = "question",    width = 3 }
DEC  = { kind = "decision",    width = 4 }
AC   = { kind = "criterion",   width = 2, scope = "feature" }

[decision_records]
prefix   = "DEC"
dir      = "docs/records/DEC"
template = "templates/decision.md"

[[generators]]
command = "spec export index"
writes  = ["docs/index.md"]
index   = true
```

| What | Where | Class |
|---|---|---|
| Rules everyone reads first | `CLAUDE.md`, Tier 0 | canon |
| The index | `docs/index.md`, built by `spec export index` | generated |
| The business-logic tree | `docs/spec/`: a root document, `<domain>/README.md`, `<domain>/<mechanic>.md` | canon |
| Atomic records | `docs/records/<PREFIX>/<ID>.md`, one file each | canon |
| Change specs | `docs/features/<slug>.md` | spec |
| Decision records | `[decision_records] dir` | decision |
| A department's practices | `docs/spec/practices/<trade>/` | canon |
| Code bindings | `spec.lock`, beside `specengine.toml` | none |
| Proposals, tasks, cards | the queue, outside the repository | none |

### One config holds everything project-specific {#GD-START-ONE-CONFIG}

Prefixes, kinds, paths, flows, departments, zones, budgets and process rules are all in `specengine.toml`. SpecEngine's code names no domain, so a word your project uses ("scene", "epic", "broadsheet") is data you declare, never a feature you wait for. A change to the config is a reviewed commit like any other.

### Files in git are the truth; the database is not {#GD-START-FILES-ARE-TRUTH}

Specs, records and `spec.lock` are files in the project's repository. The SQLite index and the queue live in SpecEngine's data directory, outside the repository; the index can be rebuilt at any time with `spec index --full`. The queue holds proposals, tasks and cards that exist nowhere else: back it up with `spec export state`.

### Read by the protocol {#GD-START-READING-PROTOCOL}

People and agents read `CLAUDE.md`, then `docs/index.md`, then at most three documents. If an answer needs a fourth read, the index or the canon is wrong: fix it rather than reading further. Long documents are read by headings first, then the one section needed.

## What not to do

### Never run without the check {#GD-START-CHECK-EVERY-COMMIT}

Run `spec check --staged` in the pre-commit hook and `spec check` in CI. `[check] mode` takes `observe`, `enforce-introduced` or `enforce` (the default); an imported corpus can start at `enforce-introduced`, so only new findings fail. An unenforced convention decays within a quarter, unnoticed until someone needs an answer.

### Never raise a budget to make a document fit {#GD-START-NO-RAISED-CAPS}

Caps (`[budgets]`) are ceilings: Tier 0 16 KB, Tier 1 and the index 10 KB, a decision 1.5 KB by default. When a document passes its cap, move detail one tier down, from `CLAUDE.md` into a folder's `README.md`, from there into a Tier 2 page. Calibrate the caps once, at adoption, from what your team actually reads.

### Never edit a generated document by hand {#GD-START-NO-HAND-INDEX}

`docs/index.md` and everything under `docs/generated/` are rebuilt by their generator. A hand edit is overwritten on the next run, and `spec check` reports the drift. Change the source front-matter, then run `spec export index`.
