---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli]
ref: RM-0076; Phase 7 step 12a (08 s2 "Step 12 in five"), split from step 12 `scale-reads` (RM-0054) by the orchestrator on 2026-11-03, the split awaiting the owner's review
adrs: []
---

# flow-linear: a flow read linear in `parent:` depth, a bundle fit that estimates each part once (RM-0076)

## Why

`spec flow` and `spec trace` find a member's parent document by building its whole `parent:` chain (`SpecGraph::ancestors`, `crates/specengine-core/src/check/spec_graph.rs:452-464`). They do it in the scope's ancestor walk, in `parented` and in `tree_of` (`crates/specengine-core/src/flow.rs:500-548`). The depth fold then climbs the tree again on every call (`shown_as`, `flow.rs:1025-1036`). A read costs O(members × depth): chains of 1 000, 2 000 and 4 000 took 0.58, 2.39 and 9.79 s in debug (`docs/canon/spec-flow.md` "Known limits"). The core test caps its chain at 1 200 for that reason (`crates/specengine-core/tests/flow_read.rs:1054-1060`). A writer's brief grows deep before it grows wide, and a hub serves every reader from one turn.

`spec bundle`'s `fits` concatenates the body with each candidate and estimates the whole text again (`crates/specengine-cli/src/bundle.rs:771-774`). That is O(candidates × body): 3 000 candidates took 1.13 s in debug (`docs/canon/spec-cli-bundle.md` "Open"). `tokens_est` adds a weight per character in thousandths and rounds up once (`crates/specengine-core/src/tokens.rs:34-40`). So the sum of the parts' thousandths gives exactly the concatenation's estimate, and running sums make the same decision without re-reading the body.

This is the first of step 12's five slices (RM-0076 to RM-0079, RM-0054). It changes no store, index or format, needs no ADR, and leaves every output byte for byte as it is.

Decided for this spec, each awaiting the owner's review:
- **L1 One parent map per read.** A document's parent document is computed at most once per read and kept in one map. The scope's walk, `parented` and `tree_of` all read that map. A `Vec` by file filled up front, O(files), is as good as filling it on demand.
- **L2 The walk stops at a walked ancestor.** Everything above a walked ancestor is already in `members` or `left_out`.
- **L3 The fold is memoised too.** The analysis missed this. `shown_as` climbs `tree.up` per call, at four call sites (`flow.rs:1044`, `:1071` twice, `:1103`). At `--depth 0` that is quadratic on its own, and the deep test reads at depth 0. Each member's shown node is computed once per read.
- **L4 Core gives the estimator's two halves.** These are `tokens::thousandths` and `tokens::tokens_of`, and `tokens_est` goes through them. A copy of the weights in the CLI would be a second rule set. Core `tokens.rs` therefore joins the files in scope, which the orchestrator's list lacked.
- **L5 Target headers stay measured per target.** `rest(index + 1)` (`bundle.rs:763-770`, `:810`) is quadratic in the number of targets alone, and the frame's 40 000 characters bound it. A suffix sum is allowed, not required.
- **L6 A golden from before.** Byte-equality is proved against outputs recorded with the binary before this slice, as `trace_views_before.json` was.
- **L7 `--trace` counts stay.** `spec flow --trace` and `spec trace` count each node's held documents again (CLI `trace.rs:579-590`, `held_documents` `:438`). That is O(nodes × held) on a deep chain, and it is not this slice's ("Out of scope").

## Description and interactions

**The flow read** (`docs/canon/spec-flow.md` "The read"; core `flow_read` `flow.rs:329`, `flow_walk` `:350`). Scope, typed edges, walk, depth, gap rows and every output are unchanged. What changes is how the scope's `parent:` ancestors, the tree's parents and the fold are found:
- A document's **parent document** stays what `parent_of` gives today (`:530-536`): the file of the first node on its chain outside its own file, or none. It is read from `SpecGraph::listed_under` (`spec_graph.rs:431`) without building the chain past that node. It is computed at most once per read (L1).
- **No ROOT.** The ancestor walk (`:500-511`) climbs the map from each member of the snapshot. It stops at the top. It stops at the first non-admitted ancestor, which joins `left_out` as today. It stops at the first ancestor already walked in this read (L2).
- `parented` (`:512-514`) and `tree_of` (`:539-548`) read the map.
- **A ROOT read's subtree walk** (`:454-479`) is already linear and stays.
- **The fold** (L3). A member at `level ≤ depth`, or with no `up`, shows as itself. Any other member shows as its `up`'s shown node. This is computed in level order, once per read, where `shown_as` climbs today.
- **Why the map equals the node walk.** After `break_cycles` (`spec_graph.rs:962`), `listed_under` is acyclic. A cycle is always cut at a document, its smallest member, since a cycle that enters a file climbs to that file's document. So the files on a chain form one run per file, each ending at that file's document. The document-level map visits exactly the files the node-level walk visits, in the same order, and `at.file == member` never holds past the start.
- **Cost.** O(files + members + links) per read, where today it is O((members + links) × depth).

Its surfaces: `spec flow` (text, `--json`, `--trace`), MCP `get_flow`, the daemon's flow route, and `spec trace`'s lanes (CLI `trace.rs:764`). All stay byte-equal.

**The bundle fit** (`docs/canon/spec-cli-bundle.md` "Fitting"; CLI `assemble`, `bundle.rs:711-904`). The order, the frame, the minimum, target forms, the greedy pass, the tail and every decision are unchanged. What changes is how a decision is computed:
- `assemble` keeps the body's measure, its characters and thousandths, beside `body`. Each `push_str` onto `body` adds its part's measure.
- Each call that tests a candidate (`:818`, `:841`, `:876`) measures its new parts once: a target's form, `after`, the separator, a candidate's heading and text, the reserve, a tail line and the more line. It then decides over the body's measure plus theirs.
- `minimum` (`:782`) and `tokens` (`:893`) equal `tokens_est` of the frame and of the body. The running measure gives the same value (L4).
- **Cost.** O(characters offered), plus `rest`'s per-target measure (L5), where today it is O(candidates × body).

Its surfaces: `spec bundle` (text, `--json`), the task package (`package.rs`), MCP `get_context_bundle`, the daemon's bundle route and eval's W (`specengine-eval` `w.rs`). All stay byte-equal, so W and every pinned bundle stay too.

## Data

No file format, table, JSON key, flag, `specengine.toml` key, environment variable, route or fixture changes, and `INDEX_FORMAT` stays 15. In code, the names are suggestions; the shapes are not:

```rust
// core src/tokens.rs: tokens_est's two halves; tokens_est(t) == tokens_of(thousandths(t)) for every t
/// Σ weight(char) of `text`, in thousandths of a token, saturating.
pub fn thousandths(text: &str) -> u64;
/// Thousandths rounded up once to tokens, saturating at `u32::MAX`.
pub fn tokens_of(thousandths: u64) -> u32;

// cli src/bundle.rs
/// What a text adds to the body.
#[derive(Clone, Copy, Default)]
struct Measure { chars: usize, thousandths: u64 }
impl Measure {
    fn of(text: &str) -> Self;          // the one place a part is estimated; #[cfg(test)]: counts its characters
    fn plus(self, other: Self) -> Self; // both fields saturating
    fn fits(self, budget: u32) -> bool; // chars <= OUTPUT_CAP_CHARS && tokens_of(thousandths) <= budget
}

// core src/flow.rs: each document's parent document (today's `parent_of`), once per read
let parents: Vec<Option<usize>> = /* by file */;
let shown: BTreeMap<usize, usize> = /* member -> shown node at `depth`, in level order */;
```

**The decision, worked** (budget 2):

| Text | Characters | Thousandths | Tokens |
|---|---|---|---|
| body `abc` | 3 | 1 050 | 2 |
| part ` d` | 2 | 500 | 1 |
| their concatenation `abc d` | 5 | 1 550 | 2: fits |
| rounded per part (a mutation) | | | 2 + 1 = 3: refused |

Count characters, not bytes. `\u{0434}`, a Cyrillic letter, is 1 character, 2 bytes and 450 thousandths. `\u{4E2D}` is 1 character, 3 bytes and 1 000. `\u{1F600}` is 1 character, 4 bytes and 1 000.

**The golden**, `crates/specengine-cli/tests/flow_linear_before.json`. It is recorded with the `spec` binary built at this slice's parent commit, each run in a scratch `HOME` over a scratch copy of the fixture. Each row is one command, run once as given and once with `--json`:

```json
[
 {"fixture": "flow-relay", "args": ["flow", "--depth", "0"], "code": 0,
  "text": "...", "json": "{\"root\":null,...}"},
 {"fixture": "spec-a", "args": ["bundle", "MEC-STAMINA", "--budget", "412"], "code": 0,
  "text": "# Bundle: MEC-STAMINA\n...", "json": "{...\"bundle_hash\":\"b3:...\"...}"}
]
```

Its cases:
- `spec flow` on every flow fixture (`flow-relay`, `flow-hire`, `flow-gaps`, `trace-almanac`, `spec-c`): the whole flow and at least two ROOTs, each at `--depth 0`, `1` and unbounded.
- `spec flow --trace` and `spec trace` on `trace-almanac`.
- `spec bundle` on `spec-a` and `spec-b`, for each target set `bundle_fit.rs` uses. Budgets: the minimum, the minimum + 1, the default, and at least 20 between the minimum and the unbounded bundle's `tokens`, the boundaries where an item just fits included.

## Rules and edge cases

- WHEN a read meets a document, its parent document SHALL be the file of the first node on its chain outside its own file, as `parent_of` gives it today. It SHALL be computed at most once per read.
- WHEN the ancestor walk reaches an ancestor already walked in this read, it SHALL stop there. WHEN it reaches a non-admitted ancestor, it SHALL add it to `left_out` and stop, as today.
- A `parent:` cycle reads as today: its cut document has no parent document (`SpecGraph::cycles`).
- A `parent:` naming an ID section of another file makes that file the parent document. Nodes of a member's own file never join its walk.
- WHEN a member is folded at `--depth N`, it SHALL show as the first node on its `up` chain whose level is at most N, or as the chain's top. The fold SHALL never climb more than once per member per read.
- WHEN `assemble` decides whether parts fit, its answer SHALL equal today's concatenating `fits` for every body, parts and budget, saturation included: `chars().count() <= OUTPUT_CAP_CHARS && tokens_est(..) <= budget` over `parts.concat()`.
- The fit SHALL count characters (`chars()`), never bytes, and SHALL round once over the whole sum.
- `tokens_est` SHALL return what it returns today for every text (core `tests/tokens.rs`, `fixtures/token-calibration`).
- No output byte, exit code or JSON key SHALL change on any surface.
- ADR-0012: nothing is refused. ADR-0008: no domain word is added. ADR-0024: tests write non-ASCII as `\u{...}` escapes only.

## Acceptance criteria

The analysis numbers step 12's criteria AC-1 to AC-14; this slice owns AC-10 and AC-11. "[default, core]" means they run in the default test run of that crate. M names the mutations that must turn each red.

- [ ] AC-10 [default, core] Linear flow.
  - `a_random_flow_reads_as_its_closures_do` (400 seeded flows, `crates/specengine-core/tests/flow_read.rs:978-1050`) passes unchanged. So do the fixture tests of core and CLI `tests/flow_read.rs`, CLI `trace_views.rs` (with `trace_views_before.json`), MCP `mcp_flow.rs` and http `flow.rs`.
  - **The forest differential** (added). The 400 seeds plant one `parent:` level (`GROUP-01`), and the deep test links every layer, so neither drives the ancestor walk.
    - A `#[cfg(test)]` unit in `flow.rs` keeps today's `scope_of`, `parent_of`, `tree_of` and `shown_as` verbatim as the reference.
    - It runs 300 seeded forests: up to 60 documents, depth up to 30, each `parent:` naming a document or an ID section of it.
    - About one document in eight is Tier 3 or generated, some `parent:` cycles are planted, and flow links sit on a random subset only.
    - For each forest, the scope (`members`, `roots`, `left_out`), the tree (`parent`, `level`, `up`) and the shown node of every member at depths 0 to 3 equal the reference's. It checks the whole read and the read under each document as ROOT.
  - **The ratio**, in core `tests/flow_read.rs`. A `parent:` chain of 8 000 costs at most 8x one of 2 000, timed on `flow_read` alone with the graph built before timing.
    - It is measured unbounded and at `--depth 0`.
    - Each measure is the best of three. The sizes alternate, and the 2 000 time is floored at 50 ms.
  - **The deep test** (`a_long_ring_and_a_deep_tree_are_walked_without_recursion`, `:1059`) takes a chain of at least 20 000, with IDs at width 5. Its doc comment drops "grows with the square".
  - **The golden.** Every `flow` and `trace` row of `flow_linear_before.json` is byte-equal, its exit code included.
  - M:
    - restore the per-member `ancestors` call in `scope_of`, `parented` or `tree_of` (about 16x: the ratio goes red);
    - `shown_as` climbing per call (the ratio at depth 0 goes red);
    - the walk stopping after one parent (the forest goes red);
    - the walk passing a non-admitted ancestor (the forest goes red).
- [ ] AC-11 [default, cli] Bundle counts.
  - Every `bundle` row of `flow_linear_before.json` is byte-equal: the text, the JSON and `bundle_hash`.
  - `bundle.rs`, `bundle_fit.rs`, `bundle_config.rs`, `bundle_bindings.rs` (with `bundle_bindings_before.json`), `package.rs`, MCP `mcp_read.rs` and eval `w_cli.rs` pass unchanged.
  - **The differential.** A `#[cfg(test)]` unit in `bundle.rs` keeps the concatenating `fits` as the reference and compares `Measure` against it.
    - It draws seeded lists of 1 to 6 parts, mixing ASCII letters and digits, other ASCII, whitespace, Cyrillic, CJK and emoji.
    - The token bound: 10 000 short lists, budgets within 2 tokens of the concatenation's estimate.
    - The character bound: 400 lists of mostly multi-byte characters whose lengths fall within 3 characters of `OUTPUT_CAP_CHARS`, at a budget their tokens never reach.
    - It asserts that each bound decided both ways at least 100 times.
    - Non-ASCII is written only as `\u{...}` escapes.
  - **The halves**, in core `tests/tokens.rs`.
    - `tokens_est(t) == tokens_of(thousandths(t))` on the fixture texts the file already reads and on seeded texts.
    - `thousandths(a + b) == thousandths(a) + thousandths(b)`.
    - `tokens_of` gives 0 for 0, 1 for 1, 1 for 1 000, 2 for 1 001 and `u32::MAX` for `u64::MAX`.
  - **The counter.** A `#[cfg(test)]` thread-local counter in `Measure::of` adds the characters it measures.
    - It is read after `assemble` runs in memory over one target and 12 000 questions on it (`bundle_layers` over a `CheckInput`, as `bundle_fit.rs` `ac09_` writes 1 000), at a budget that places some and refuses others.
    - The counter is at least the candidates' characters.
    - It is at most twice the characters of the frame, the reserve, the target's forms, the candidates' texts with their headings, and the tail's lines with the more line.
  - M:
    - count bytes instead of characters (the differential goes red);
    - round per part (the differential and the golden go red);
    - restore the concatenating `fits` through `Measure::of` (the counter's upper bound goes red) or through `tokens_est` (its lower bound goes red).
- [ ] F1 — form. `cargo clippy -p specengine-core -p specengine-cli --all-targets` is clean, and the implementation's `git diff` touches no `docs/`, fixture or `Cargo.toml`. `export index` and `check` give 0 errors and 0 warnings.

## Out of scope

- The store, its index and `INDEX_FORMAT`: RM-0078.
- Reads from the index and the per-read re-parse: RM-0079.
- Read freshness and a hub's trusted index: RM-0054.
- The synthetic corpus and the 100 000-node bars: RM-0077, RM-0054.
- The `--trace` counts (L7): `spec flow --trace` and `spec trace` stay O(nodes × held) on a deep chain, and canon keeps that as a known limit.
- `rest`'s per-target measure (L5).
- `SpecGraph::ancestors` and its other caller, the bundle's ancestors layer (core `check/bundle.rs:218`).
- Uncut gap rows (`spec-flow.md` "Known limits").
- `spec check`'s own-text cost (`spec-check-process.md`).
- Token weights and the tokenizer.

## Implementation

To fill in after implementation. Code comments cite the canon (`docs/canon/spec-flow.md` "Code", `docs/canon/spec-cli-bundle.md` "Fitting") or this spec's "Acceptance criteria", the headings that survive compaction.

### Files in scope

| Module | What it changes |
|---|---|
| core `src/flow.rs` | the parent map (L1); the walk stopping at a walked ancestor (L2); `parented`, `tree_of` reading the map; the fold memoised (L3); `parent_of` gone or O(1) |
| core `src/check/spec_graph.rs` | only if wanted, an accessor for the first node outside a file; `ancestors` unchanged |
| core `src/tokens.rs` | `thousandths`, `tokens_of`; `tokens_est` through them (L4) |
| cli `src/bundle.rs` | `Measure`; `assemble`'s running body measure and its decisions; the module doc's **Budget** bullet |
| Units (`#[cfg(test)]` in `flow.rs`, `bundle.rs`) | the forest differential with its reference; the fit differential; the counter |
| core `tests/flow_read.rs`, `tests/tokens.rs` | the 20 000 chain and the ratio; the halves |
| cli `tests/flow_linear.rs`, `tests/flow_linear_before.json` (new) | the golden |

The rust-developer writes `src/`. The test-engineer writes the units, the tests and the golden; the golden is recorded before `src/` changes, which lets the two run side by side. No `Cargo.toml` change.

### Canon at ship

Nothing below is written before the ship commit. Free bytes are as of this spec; no cap is raised.

- **`docs/canon/spec-flow.md`** (114 B free):
  - "Known limits" `:98` becomes "- `--trace` and `spec trace` count each node's held documents again (`trace.rs` `counts`): quadratic in `parent:` depth." (+2 B).
  - "Code" `:92`'s `flow_read` sentence gains "; one parent map and one fold per read keep it linear in `parent:` depth" (+72 B).
  - "Tests" `:103` gains ", CLI `flow_linear.rs`" (+22 B).
- **`docs/canon/spec-cli-bundle.md`** (11 B free):
  - "Open" `:98` becomes "- Each call re-parses every file (pass 3), MCP inheriting it (`docs/canon/mcp-read.md` "Latency"): RM-0079." (−168 B).
  - "Fitting" `:43`'s **Fits** gains " Running sums of characters and thousandths make that decision, rounding once (`tokens_of`): each part is estimated once." (+121 B).
  - "API" `:90`'s tests gain "`flow_linear.rs` (the bytes before)" (+37 B).
- **Tier 1:** in the core README (8 B free), "Token estimator" `:53` opens "`tokens_est = tokens_of(thousandths)`: Σ weight(char), thousandths per class: ..., other 1000; rounded up once, saturating `u32`; a document costs its file, a section its span. Calibrated on `claude-opus-5-5` (2026-10-04, ...". That is +5 B, the rest of the line as it is. The CLI README is unchanged.
- **`CLAUDE.md`:** the state line names 12a `flow-linear` shipped and 12b next.
- **Plan:** RM-0076 shipped and compressed; 08's row 12a shipped; this file compressed.
- **Unchanged:** `architecture.md` (no rule changes), every ADR, the store README, `mcp-read.md` "Latency" (a measurement of its date).
