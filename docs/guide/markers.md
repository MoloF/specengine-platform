---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-20
summarises: [docs/canon/architecture-code.md#markers, docs/canon/architecture-code.md#spec-lock, docs/canon/architecture-code.md#verify, docs/canon/architecture-code.md#sync-in-reads, docs/canon/code-bindings.md#the-code-table, docs/canon/spec-verify.md#states, docs/canon/proposal-drift.md#raise]
---

# Code markers and the lock

A marker is a one-line comment binding Rust or RON code to the spec node it implements, and `spec.lock` records the pair's hashes as the owner verified them. This page covers writing markers, choosing their levels, reading drift with `spec verify`, and who writes the lock.

## What it solves

Spec and code drift apart quietly: a rule changes and nobody knows which systems to revisit; a system is refactored and nobody re-reads the rule. A marker names the code's node and the revision it was written against. The lock keeps a verified state of each pair, so SpecEngine can say exactly which side moved since, without blocking anyone.

## When to use it

- **New work**: an agent or developer adds a marker to each item that implements, tests or configures a node.
- **Touching old code**: add the marker then. Existing ID mentions in comments stay weak `mentions`; there is no mass conversion.
- **After a rule changes**: find the code that lags; after code changes, find the rules to re-read.

## How

### CLI

A marker is a comment on the item it binds, usually the line above it:

```rust
// @implements RULE-STAM-REGEN@2 [sig, body]
pub fn regen_system(mut q: Query<&mut Stamina>, time: Res<Time>) { ... }

// @verifies EDGE-STAM-ZERO@1 [body] drained stamina applies Exhausted at once
#[test]
fn zero_stamina_exhausts() { ... }
```

The keywords are `@implements`, `@verifies`, `@configures` and `@assumes`. `@2` is the node's revision the code matches. The levels in brackets say which changes of the item count: `path`, `sig`, `body`, `deps`; none written means `[sig, body]`.

```text
$ spec refs RULE-STAM-REGEN
$ spec symbols --node MEC-STAMINA
$ spec unmapped src/
$ spec verify
$ spec lock accept RULE-STAM-REGEN
```

`spec refs` lists the code bound to a node; `spec symbols` and `spec unmapped` look the other way, from code to nodes and to unbound items. `spec verify` prints one row per binding with its state: `ok`, `spec_ahead` (the rule changed, the code did not), `code_ahead` (the code changed, the rule did not), `conflict` (both), `broken_binding`, `bad_marker` or `cannot_verify`. The owner records a verified pair with `spec lock accept` on a terminal. `spec propose drift` queues current drift as items for the owner; `spec health` and `spec impact` show what a change reaches.

### MCP

`find_symbols`, `refs`, `unmapped` and `check_binding` read the code side; `get_impact` shows what a node, a symbol or the code since a revision reaches; `report_drift` queues drift. `get_node` with `with: ["bindings"]` and `get_context_bundle` show each binding's `sync`, the same state `spec verify` would give now.

### Screen

A node's Bindings tab lists its code and each binding's sync, with "Changes since the lock" when code moved. Health shows drift and unbound nodes as counts with their causes.

## Where it lives

The code layer is off until `specengine.toml` has a `[code]` table:

```toml
[code]
roots      = ["crates", "assets/data"]
exclude    = ["**/target/**"]
frameworks = ["bevy"]          # optional: ECS roles of items

[verify]
mode = "observe"               # observe (default) | enforce-introduced | enforce
```

`spec.lock` sits beside `specengine.toml` and is committed. Each entry pairs a node and revision with a symbol and its hashes:

```toml
[[binding]]
node = "RULE-STAM-REGEN"
rev = 2
relation = "implements"
file = "crates/game/src/stamina.rs"
symbol = "game::stamina::regen_system"
levels = ["sig", "body"]
# ... the hashes, the file's blob and the commit
verified_by = "owner"
```

### Bind new work with a marker {#GD-MARKERS-BIND-NEW-WORK}

Every item that implements, verifies, configures or assumes a node gets a marker when it is written or touched. Cite the node's current revision, `ID@rev`. A working answer the code relies on is marked `// @assumes PR-0009`, so the code can be found when the question is decided.

### Raise the rule, then align the code {#GD-MARKERS-RAISE-THEN-ALIGN}

A change in a rule's meaning raises its revision (`guide:records#GD-RECORDS-REVISION`). The markers citing the old revision then show `spec_ahead`: bring the code in line, raise the marker to the new revision, and have the owner run `spec lock accept`. Never raise a marker before the code matches the rule.

### Drift is shown, never blocking {#GD-MARKERS-DRIFT-SHOWN}

`spec verify` reports and stops nothing unless your own `[verify] mode` makes CI or a hook fail on it. Reads show `sync` and Health counts drift; no task, card, approval or gate waits on them. A binding that cannot be verified is never shown as `ok`.

### Only the owner writes the lock {#GD-MARKERS-OWNER-LOCK}

`spec.lock` is written by the owner's terminal alone: `spec lock accept`, approving a drift item, `spec lock regen` after a merge, `spec lock rebase` for a new build, an editorial approve, and `spec verify --tests` for test evidence. Each is its own commit with provenance.

## What not to do

### Never edit `spec.lock` by hand {#GD-MARKERS-NO-HAND-LOCK}

No agent, tool, role or hand edit writes the lock; it would vouch for a pair nobody verified. On a merge conflict in `spec.lock`, run `spec lock regen` instead of resolving it by hand.

### Never mass-convert old citations {#GD-MARKERS-NO-MASS-CONVERSION}

A comment that merely mentions an ID is a weak mention and stays one. Turning hundreds of mentions into markers at once binds code nobody checked against the rule; add markers as you touch the code.

### Never let an agent mark tests verified {#GD-MARKERS-TESTS-BY-OWNER}

A test counts as evidence only when the project's own test command runs it on the owner's terminal (`spec verify --tests`), never from an agent's report that the tests passed.
