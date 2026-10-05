---
class: spec
status: in-progress
scope: [specengine]
ref: research-2026-09-28
---

# 05. Target architecture of SpecEngine

> Recommended architecture after the research. Deviations from the initial spec are justified in `03-critique-of-initial-spec.md`.
> Accepted decisions live in `docs/decisions/ADR-NNNN.md`; the rules that follow from them are in `docs/canon/architecture.md`.

## 1. Topology

```
                ┌────────────────────────── project repository (git, worktrees) ──────────────────────────┐
                │  docs/spec/**.md   docs/records/**.md   spec.lock   specengine.toml                     │
                │  src/**.rs (// @implements R-12)   tests/**.rs (// @verifies AC-07)   data/**.ron       │
                └───────────────▲──────────────────────────────▲───────────────────────────▲──────────────┘
                     reads/writes │ (apply_proposal — one door)    │ tree-sitter              │ git CLI
                                 │                               │                           │
┌────────────────────────────────┴───────────────────────────────┴───────────────────────────┴──────────────┐
│ specengine (one Rust binary)                                                                             │
│                                                                                                          │
│  core:  parser(md+yaml) · refs · records · graph · checks · budget · bundle · proposals · tasks          │
│  code:  tree-sitter · RON lexer · module resolver · symbol index · AST canonical hash · bevy detector    │
│  store: SQLite (WAL) = index (rebuildable) + operational state (queue, tasks, runs)                      │
│                                                                                                          │
│  adapters:  CLI (clap) │ MCP stdio │ HTTP daemon: REST + SSE + MCP Streamable HTTP + static Web UI       │
└──────────────▲──────────────────────────────▲─────────────────────────────▲──────────────────────────────┘
               │ owner's terminal              │ Claude Code (agents,         │ owner's browser
               │ spec inbox / review / task    │ hooks, /feature)             │ tree, graph, queue, diff
```

Principles:

1. **One core, four adapters.** CLI, MCP, HTTP and the CI check call the same core functions, so their results cannot diverge: the gate and the editor show the same numbers.
2. **Git is the truth for specs and bindings; SQLite is the index and the work queue** (see 03 §2.1; the DB: `crates/specengine-cli/README.md`).
3. **One writing door** into spec files — `apply_proposal`, triggered by a human (from the UI, the CLI or via MCP `elicitation`). Agents read and propose but never write (ADR-0004).
4. **Local and native.** The daemon listens on `127.0.0.1` only. Docker is a launch option, not the primary mode.
5. **Several projects in one daemon**: consumer projects are registered by path. The shared library (§3.6) is a separate project they reference.
6. **The daemon is the sole SQLite writer** (tracey topology, 04 §1.5). `spec mcp` (stdio), the CLI and hooks are thin bridges to the daemon over a Unix socket. If the daemon is not running, the bridge starts it; after idling it exits. No races between processes for the DB, no re-reading the model on every call (Reqvire lesson: 17–80 s per read).

## 2. File layout in the project repository

Now canon: `docs/canon/architecture.md#layout` (granularity, the default directories; ADR-0026); `[paths]`: `crates/specengine-core/README.md`; the import's projection into it: `docs/canon/import-layout.md`.

### 2.1. Mechanic example

`fixtures/spec-a/docs/spec/movement/stamina.md`: front-matter with `parent` and `links:` (`derived_from`, `depends_on`, `uses_term`), an H1 whose first paragraph is the summary, rules as `## Regeneration {#RULE-STAM-REGEN}` sections.

### 2.2. Question record example

`fixtures/spec-a/docs/records/Q/Q-031.md` (values in comments); rules: `docs/canon/spec-check-process.md`.

## 3. Data model

### 3.1. Node kinds (`kind`)

A kind is project vocabulary, named per prefix in `[ids]`: the core knows none (ADR-0008, ADR-0031). Usual kinds: `game`, `domain`, `mechanic`, `rule`, `invariant`, `edge_case`, `data_contract`, `requirement`, `assumption`, `decision`, `principle`, `term`, `question`, `feature`, `criterion`, `check`, `generated`. IDs are Latin (ADR-0009): a look-alike letter gives `homoglyph` with the Latin fix as data (`spec check` rejects; only `apply_proposal` writes, ADR-0004); legacy IDs resolve through `aliases_from` / `aliases`; `spec new` issues numbers. `[ids]` and the grammar: `crates/specengine-model/README.md`; the parser: §3.2.

### 3.2. Link types

Link types and the one grammar for text, markers and search: `crates/specengine-model/README.md`; declaring keys, parser caps: `crates/specengine-core/README.md`; containment, graph and impact reads: `docs/canon/spec-cli-graph.md`.

### 3.3. Index schema (SQLite)

The rebuildable part (deleted and regenerated by `spec index`) is shipped for the spec corpus: `crates/specengine-store/README.md`. Still to come: `norm_hash` (§3.5, 08 AC-13); Phase 3 `symbols` (file, qpath, kind, layer, signature, `ast_hash`, lines) and `markers` (node, qpath, relation `implements|verifies|configures`, file, line).

Operational tables (not recoverable from git, §8): `proposals` and `events`, shipped for kind `update` (`docs/canon/proposal-queue.md` "Store"). Later slices add kinds `discrepancy|question|create|decision|interpretation|amendment`, states `changes_requested|deferred|superseded`, `gap_type` (`missing|partial|contradicts|unrequested`), `severity` (`high|normal|low`: queue order, never blocking, ADR-0012), `task_id`, several targets, `evidence` (`[{file, qpath, lines, observed, documented}]`), `options` (`[{label, effect, price}]` + recommendation), `working_answer`, the author's `cost`, `proposal_comments (id, proposal_id, author, body, created_at)`. To come:

```sql
CREATE TABLE tasks (
  id TEXT PRIMARY KEY,               -- T-0107
  project TEXT, title TEXT, goal TEXT,
  status TEXT,                       -- draft|analysis|review|changes_requested|ready|in_progress|in_review|done|accepted|cancelled (no blocked, ADR-0012)
  stale INT,                         -- informational flag: a node from spec_snapshot changed
  priority INT, contour TEXT,        -- task contour; vocabulary set by specengine.toml (e.g. content|feature)
  target_ids TEXT, criteria_ids TEXT,
  spec_snapshot TEXT,                -- JSON: node_id → norm_hash, frozen on transition to ready
  brief TEXT,                        -- generated assignment
  worktree TEXT, branch TEXT, created_by TEXT, created_at TEXT, updated_at TEXT);
CREATE TABLE runs     (id, task_id, role, model, started_at, ended_at, cost, bundle_hash, artifacts_path, outcome);
CREATE TABLE bundles  (hash PRIMARY KEY, task_id, node_ids, tokens, budget, created_at, body); -- determinism, audit
```

### 3.4. Status axes

- `spec_status` (front-matter): `draft → review → accepted → superseded | rejected`.
- `impl_status` (lock/index): `none → planned → in_progress → implemented → verified`.
- `sync` — **computed only** (§5.3).
- `acceptance` (feature): `pending → accepted`.
- `has_open_proposal` — flag from the queue: the node is "in question"; the context bundle shows it.

### 3.5. `spec.lock`

```toml
recipe = 1                  # normalization recipe version: a better normalizer = an honest rebase

[[binding]]
node = "RULE-STAM-REGEN"
rev = 3                     # node revision the binding was verified against
symbol = "movement::stamina::regen_system"
relation = "implements"
file = "src/movement/stamina.rs"
file_blob = "5f1c9e…"       # git blob OID of the file: unchanged → binding fresh in O(1), no parsing
spec_hash = "b3:9f2c…"      # norm_hash of the node
ast_hash  = "b3:41aa…"      # canonical symbol hash
commit    = "a0cea48"
verified_by = "owner"       # owner | ci — review record: who, when, against which revision
verified_at = "2026-09-28"
```

The file is generated, committed together with the code, and conflicts in it are resolved by `spec lock --regen`. It turns "the spec matches the code" into a **verifiable fact at a specific commit**.

**What goes into `norm_hash`** (after Doorstop): the section text and semantic fields (`links`, for records `working_answer` etc.). **Excluded** are presentational and process fields: `title`, `status`, `tier`, `owner`, `reviewed`. Renaming a heading must not invalidate coverage.

**Node revision `rev`** (after tracey and OpenFastTrace): a monotonic number in front-matter. `spec check --staged` in pre-commit **fails if `norm_hash` changed but `rev` did not**. `spec bump` raises the revisions of changed nodes in the git index; `spec bump --editorial ID` marks the edit as editorial and updates `spec_hash` in the lock without raising the revision. "Rewrote the meaning, coverage stayed green" becomes impossible, and typos do not create false drift.

### 3.6. Shared library

A separate project `shared`: Bevy 0.19 conventions, test rules (probes, named mutations, "no mocks"), RON data rules, common glossary. A project node references it via `links: { adopts: [shared:PAT-PROBES@rev] }` with a pinned revision. When `shared` changes, SpecEngine shows the project an "update available" notice but never applies it itself.

## 4. Checks (`spec check`)

Canon: `docs/canon/spec-check*.md`; pending: 08 Phase 1; queue state → Phase 2; code → Phase 3.

## 5. Code: symbol index, bindings, drift

### 5.1. Symbol index: three layers

**Layer A (always, MVP) — tree-sitter.** Fast, incremental, no build: markers, item spans, canonical AST hash, heuristic `qpath`. If `qpath` is ambiguous (adjacent `impl`s, `#[path]`, macros), the state is `cannot_verify`, not a guess. This is enough because **link identity comes from the marker in code**, not from the symbol path.

**Layer B (Phase 3) — Bevy's own schedule graph.** Bevy 0.19 `bevy_dev_tools::schedule_data` (features `debug` + `schedule_data`): `SerializeSchedulesPlugin` writes `app_data.ron` — schedules with their systems (full `type_name` paths), sets, ordering edges, conditions, access conflicts, `apply_deferred` sync points; **no observers and no plugins**. It is **the only mechanism that sees systems from macros, loops, generic instances and third-party plugins**, so it is the truth for registrations and layer A is reconciled against it. Obtaining it (verified on both pilots): a 6-line patch in 2 files (the two features, winit off, `ScheduleRunnerPlugin::run_once()`, `SerializeSchedulesPlugin`) on a scratch copy with its own `CARGO_TARGET_DIR`; a headless run of seconds writes the dump before the first frame, so assets may be left out; `HOME` points at a scratch directory so the game cannot touch the user's settings. The typed reader runs over the RON lexer; fields outside the pinned Bevy schema are counted, not fatal — the schema is unstable across minors, re-check on every Bevy upgrade. `bevy_mod_debugdump` stays a fallback (not needed so far). Reconciliation is one-to-one by schedule and terminal name (generic arguments dropped, a closure → its `fn`, `Pipe(a, b)` → `a`); every other dumped own-crate system is a miss with a named category.

**Layer A alone covers 93.4–100 %** of dumped own-crate systems on the pilots (threshold 95 %, counted per system, not per registration site); every miss was a `generic_instance` — instances of one generic system from a single site. Layer A cannot enumerate them, so layer B or C stays required (ADR-0020).

**Layer C (ADR-0020) — rust-analyzer as a library** (04 §1.7), crate `specengine-ra`, outside the core build graph: for what layer B does not cover (components, plain functions, methods). Entry point **`MonikerResult::from_def`** for the needed definitions only, not `StaticIndex::compute`, which builds hover and docs for every token; type inference (~68 % of analysis time) is not needed. The `ra_ap_*` set is pinned to one **`=0.0.352`** and needs rustc ≥ 1.98 (04 §6). Result: a resolved moniker in SCIP grammar with two amendments (package version → `.`, `disambiguator` for adjacent inherent impls) and Bevy system registration by resolved types via HIR (`Semantics::resolve_method_call`). A load: `cargo metadata` (`--locked --offline`) → build scripts and proc-macro dylibs (`cargo check --compile-time-deps`) → database + proc-macro server, in its own `CARGO_TARGET_DIR`; it runs the project's own build scripts and proc macros. **Measured on both pilots** (release, proc-macro server on): cold load 33–65 s (8–10 s without the server), warm full pass ≈ 0.2 s, whole-process-group peak ≤ 3.76 GiB on the heavier pilot (≈ 6 % under the 4 GiB threshold), no crash, timeout or panic, monikers on 99.7–100 % of items. **Open for Phase 3**: items under attribute proc macros (`#[tokio::main]`-style) lose their moniker with the server — the scan does not map an item through its attribute expansion. In the daemon the layer runs in the background with debounce. **An LSP client is not used for identity**: LSP has no monikers.

Layer A details:

- **Items and `qpath`** (Cargo target units, method names, ambiguity): `docs/canon/code-identity.md`; following `mod x;` is layer C. Macro-generated items are invisible, a stated limitation.
- **Bevy detector** (`specengine-code`; the node layer is inferred, not written by hand):
  - `#[derive(Component)]`, `#[derive(Resource)]` (in 0.19 also a `Component`), `#[derive(Message)]`, `#[derive(Event)]`/`EntityEvent`, `#[derive(Reflect)]`; `.add_message::<T>()` → message registration;
  - systems of `.add_systems(Schedule, …)` and of the one-argument `Schedule::add_systems(…)`: tuples nest (explicit stack, cap 256), combinators `.in_set/.before/.after/*_ignore_deferred/.run_if/.distributive_run_if/.ambiguous_with*/.chain*` are peeled, adapters `pipe/map/with_input/with_input_from` recorded; a leaf is a path (`tick`, `a::b`, `f::<T>`), a closure (named by its innermost enclosing `fn`) or a factory call (named by its callee); `.add_observer(…)` → `observer`; `.add_plugins(…)` → plugin uses;
  - plugins: every `impl Plugin for X` and **every function whose only parameter is `&mut App`** (qualified or generic; not a `self` method, not `-> &mut App`);
  - the same calls inside `macro_rules!` transcribers and macro arguments are read from tokens: names only, never resolved or matched against a dump; the token reader is approximate (a comma in a type position or closure parameters splits an element; trailing tokens after a path → `expression`), ordinary code is read exactly from the syntax tree;
  - what it cannot read is `uncertain` with a category (`macro_in_arguments`, `metavariable`, `unknown_method`, `expression`, `arguments`, `nesting_too_deep`, `parse_error`), never a guessed name; texts capped at 128 bytes; cost linear, no recursion over input.
- **Signature** — the text before the body (`fn regen_system(q: Query<…>, time: Res<Time>)`) goes into the context bundle instead of the body. It is a targeted "repo map": unlike Aider's, it sees systems that are merely registered.
- Detector boundaries: registration via `inventory`/`linkme`/`bevy_auto_plugin` and hierarchies inside `bsn!` are invisible (declared limitations); completeness is checked by layer B. The `add_systems` signature is unchanged from 0.16 to 0.19.1; in 0.20-rc observers change again (`On<Add<A>>`), so observer reading goes through a Bevy-version gate.
- **User anchors** (option): instead of a marker a node may reference an ast-grep rule (`anchor: { rule: "...", inside: "mod player" }`). Needed for code where a marker cannot be placed, e.g. macros.

### 5.2. Canonical AST hash

Recipe **`specengine-hash/v2`** (`specengine-code`), assembled from limpet (`src/memory/anchor.rs`), mago-fingerprint and the research (04 §1.8), then amended by measurement:

```
hash(item) = BLAKE3( header ‖ walk(attrs(item)) ‖ walk(item, skip = name_node(item)) )
header     = "specengine-hash/v2" ‖ tree-sitter-rust version ‖ abi_version()
walk(n) = if n.kind() ∈ {line_comment, block_comment} → ""   // by kind(), never is_extra(): trap 1
          else if n is anonymous "," → ""                     // rustfmt trailing commas
          else if n matches N1–N4 → walk(normal form of n)
          else if n is leaf → len‖kind(n) ‖ len‖text(n)       // len = u32 little-endian
          else → "(" ‖ len‖kind(n) ‖ concat(walk(c) for c in children) ‖ ")"
item.has_error() → cannot_verify, no hash
```

`attrs(item)` is the attached attribute run: tree-sitter-rust exposes attributes as siblings of the item. The walk is iterative (explicit stack). **Normalisations** — exactly the four constructs rustfmt toggles:

- **N1** a `;` after an absent or diverging tail is transparent (edition 2024 adds it after diverging tails);
- **N2** a label-free single-expression block in a closure body or a match-arm value is unwrapped (`|x| { x + 1 }` = `|x| x + 1`);
- **N3** `{…}` token trees after `|`, `||` or `=>` inside `(`/`[`-delimited expression macros are transparent;
- **N4** a run of adjacent `use` items is sorted, each member = its attached attributes ‖ its `use`, so attributes travel with their `use`; the member walk is bounded at `MAX_USE_RUN_NESTING = 64` runs, past it → `cannot_verify` (`nesting_too_deep`), no hash.

**Measured** (`specengine-eval ast-hash`, both pilots, 11 852 + 7 599 items): v2 is 100.0 % stable under default rustfmt, a contrasting rustfmt (`max_width = 60`, rewrites almost every file) and comment stripping including `///`; broken items never share a hash. The v1 formula (no N1–N4) is refuted: 84.9–85.4 % under the contrasting format. **Known limits**: N3 hashes token-level DSL macros the same with and without their significant `| {` / `=> {` braces (rustfmt never formats such macros; remedy — a per-project opt-out list of macro names in `specengine.toml`, not built yet), and a nested brace-delimited macro inside an expression macro inherits N3; `mod x;` and `extern crate` runs reordered by rustfmt are not sorted (v3 candidate).

- **The symbol's own name is not hashed** (limpet technique): a pure rename does not change the body hash, and a renamed symbol can be found by body match. Identifiers inside the body do count.
- **Fingerprint levels** (lockwire idea): a symbol has four independent hashes — `path` (location), `sig` (name-independent signature: parameters, types, return, modifiers, attributes), `body` (normalized body via the `body` field of `function_item`/`impl_item`/`trait_item`), `deps` (sorted set of outgoing calls). **A binding declares which levels it depends on**: an interface requirement — `sig`, an invariant — `body` + `deps`, a "lives here" note — `path`. Marker: `// @implements RULE-X@3 [sig]`, default `[sig, body]`. This is the main way to keep every refactor from becoming an alarm the owner learns to ignore.
- Whitespace is not a node in the Rust grammar at all; together with the dropped `,` and N1–N4 this gives formatting insensitivity. Acceptance check: **hashes of all symbols in the project are identical before and after `cargo fmt`** — re-run with `specengine-eval ast-hash` on every grammar bump.
- **Doc comments** (`line_comment` with the `doc` field) are prose and are not hashed; they are structurally distinguishable. **Attributes** (`#[derive]`, `#[cfg]`, `#[require(..)]`) are real nodes and **are hashed**: they change behaviour.
- Only `kind()` strings are used, **never `kind_id()`/`grammar_id()`**: these are indices into generated tables and would show drift everywhere after a grammar update. The grammar version is written into the recipe header, so a grammar change is a deliberate rebase, not false drift.
- `to_sexp()` as hash input **does not work**: it drops anonymous nodes and leaf text (`a+b` = `a-b`). `Node` hashes by pointer; do not use `#[derive(Hash)]`.
- Option `--deep`: hashes of called functions from the same crate are added, depth 1.

**Five traps that silently break a naive implementation** (trap 1 reproduced, the rest are research measurements):

1. **`is_extra()` returns true for ERROR nodes.** The filter `if node.is_extra() { skip }` drops the whole erroneous subtree, and every unparsed file gets **the same hash**: the detector goes blind for good. Correct: filter comments by `kind()` (or `is_extra() && !is_error()`) and **check `has_error()` at item level**. If an item has a parse error, the state is `cannot_verify` ("hash unreliable").
2. **tree-sitter-rust 0.24.2 fails on some valid code** (macro punctuation `$`/`~`, multi-line `where`; research estimate ≈ 1 %, 0 items on both pilots). Recovery is local, so the remaining items of the file hash normally.
3. `to_sexp()` is unusable (above).
4. `kind_id()` is unstable across grammar versions (above).
5. Aliases: `kind()` returns the public name, `grammar_name()` the raw one. Use `kind()`.

**Following renames and moves — the anchor-state vocabulary of code-graph-rag (cgr)**, which agents already see via MCP. No need to invent our own:

| Anchor state | When |
|---|---|
| `EXACT` | marker in place, symbol found (reverting the edit returns the state to `EXACT`) |
| `MOVED` (+ `moved_from`) | marker lost, but exactly one definition in the project matches by fingerprint; the binding follows it |
| `AMBIGUOUS` (+ `candidates`) | several definitions match; the owner decides |
| `LOST` | nothing found |

cgr's `STALE` (body changed) is our `code_ahead`. The anchor state answers "where is the symbol"; `sync` (§5.3) answers "does the meaning still match".

To find a symbol after a rename and to tell copies apart, **three fingerprints are needed, not one** (cgr technique):
1. `body_hash` (§5.2) — for drift: identifiers inside the body count;
2. **skeleton fingerprint** — all names, literals and comments masked, pure structure remains. Survives a rename but coincides for copies. Separate fingerprints **per branch** (statement) give a change ratio ("2 of 6 branches changed") instead of a boolean;
3. **textual anchor quote** — a digest of the definition text with the name masked plus the lines before and after. Distinguishes copies with the same skeleton.

Triviality threshold (limpet): a body shorter than ~124 bytes of buffer is **not followed** through renames — empty functions and one-line delegates are "twins", and Bevy has many (`fn build(&self, app) { app.add_systems(..) }`). Principle: "a missed follow heals as STALE, a false follow lies forever". A rename is detected as the pair "symbol vanished + a symbol with the same skeleton appeared in the same file" (one to one). With identical `qpath`s a discriminator applies, and resolution does not depend on row order in the DB.

**`qpath` lookup** — through a suffix index from day one: in cgr a linear scan of the name registry ate 48 % CPU; the index sped this up 178–382×. `find_symbols` accepts both a name and `path:line` (returns the definitions covering the line, innermost first); ranking: exact → path suffix → same name. Every resolved link carries a resolution confidence (`exact | heuristic`), and destructive operations (rebinding, ID rename) **refuse** to act on a heuristic without an explicit flag.

**Reply to every write** (cgr "structural delta" contract): `spec verify`/`reindex` return `{symbols: {added, removed, renamed, changed}, bindings: {moved, ambiguous, lost, stale}, affected_nodes, tests_reaching, ms}`. `spec check --base <ref>` returns the same for pre-commit and CI.

**Performance is not a concern** (research measurement on a corpus of several hundred thousand lines, 14 cores): parse + hash runs at ~112 MB/s with rayon and one `Parser` per thread (`thread_local!`); half a million lines hash in a fraction of a second. Caching and incremental parsing are unnecessary for the hash layer; all the complexity goes into correctness.

**`syn` 3 + `extra-traits` is not the digest** (ADR-0021 stands): structural `Hash` and no span leakage, but no error recovery (whole file or nothing), a trailing comma and `///` → `#[doc]` change the hash, and even after normalisation it reached only 94.5–94.9 % stability on the pilots; its speed (time ratio ≈ 0.55) does not decide. It stays only as the comparison behind `specengine-eval`'s `syn` feature.

- Acceptance test (from the initial spec, kept): editing a comment, whitespace or a neighbouring function in the same file does **not** change the hash.

### 5.3. Markers and the sync matrix

**Marker grammar** (`.rs` and `.ron` alike) and **RON binding** (own lexer, §9; the field path is the `qpath`, `data/movement.ron#root.stamina.regen_per_second`): `docs/canon/code-identity.md`. In Rust a marker applies to the next item; Rust binding (Phase 3) follows the RON principles.

**Revision in the marker** (`@3`, ADR-0018) — tracey discipline. First the code is brought to the new rule text, **then** the marker is raised, and the raise is visible in the code diff as a review record. Marker `@2` on a node with `rev: 3` means `spec_ahead` even without a lock. Marker `@4` on a node with `rev: 3` means `predated` (reference to a non-existent revision; OpenFastTrace reports this error separately). A marker without a revision is allowed for weak bindings and in adoption mode.

**Drift-check cascade** — the next level runs only if the previous one said "changed":

1. **git blob OID** of the file matches the lock → all bindings in the file are fresh, no parsing (Swimm `file_blobs` technique). "Which nodes could this commit touch" reduces to a set intersection of changed paths;
2. **normalized text** of the region (comments stripped, whitespace collapsed, string literals verbatim) — the fallback when the symbol does not parse;
3. **canonical AST hash** of the symbol (§5.2);
4. *(optional, later)* **LLM audit** only over nodes flagged at steps 2–3. The audit runs in a **clean context** of a separate agent without Edit/Write, and every behavioural claim must carry a `file:line` quote.

| spec_hash ≠ lock | ast_hash ≠ lock | sync | Action |
|---|---|---|---|
| no | no | `ok` | — |
| yes | no | `spec_ahead` | a "bring code to spec" task is created/updated |
| no | yes | `code_ahead` | into the owner's queue: "accept code changes" (update lock) or "reject" (task to revert/fix). If the meaning changed too, the agent proposes a spec edit |
| yes | yes | `conflict` | UI: spec-section and symbol-span diffs since `lock.commit`, side by side |
| — | symbol not found | `broken_binding` | marker removed or symbol renamed; a rebind proposal |
| node accepted, no bindings | | `unbound` | visible on the "what's left" panel |
| marker `@N` > node `rev` | | `predated` | error: reference to a non-existent revision |
| unparseable / symbol ambiguous | | `cannot_verify` | **never counted as `ok`**; shown separately |

`spec verify` verdicts follow the amiss discipline: **Fix** (broken by this change), **Check** (a human should re-read), **Pre-existing** (predates the change, goes to the backlog). Exit codes: **0** — nothing blocks, **1** — something blocks, **2** — the run cannot be trusted, no verdict. Changing spec and code in the same change is not drift; it is a `conflict` only when revisions disagree.

Git as the truth gives "since verification" diffs for free: the old spec text and the old code come from the commit recorded in the lock.

### 5.4. Tests as evidence

`spec verify --tests` collects the names of tests carrying `@verifies` for the task's nodes and runs `cargo nextest run -E 'test(=name) | …'` (or `cargo test -- --exact`). `runs` records the command, commit, result and output hash. The `implemented → verified` transition is made **by SpecEngine from the result**, not by an agent's self-report.

### 5.5. Node evidence level

Computed and shown in the UI and in the context bundle. The answer to "agents get their power from the test suite, not from prose":

| Level | Evidence |
|---|---|
| 1 | a bound **passing test** (`@verifies`) run by SpecEngine |
| 2 | a **deterministic machine assertion**: e.g. "system X in schedule Update after Y", "component Z requires W" — checked by extracting `add_systems`/`#[require]` |
| 3 | a fresh binding to a symbol (AST hash matches) |
| 4 | an LLM-audit verdict with a `file:line` quote |
| 5 | prose only |
| — | `non_verifiable`: the node is explicitly marked unverifiable (procedural principle, lore) |

The coverage requirement is set **on the node**, not globally (OpenFastTrace `Needs:`): `needs: [impl, test]` on an invariant, `needs: []` on an explanation.

## 6. Context bundle (`get_context_bundle`)

Shipped in Phase 1 as `spec bundle` (CLI pass 4): layers by link type, budget, fitting, the tail and `bundle_hash`: `docs/canon/spec-cli-bundle.md`. Not delivered yet: open proposals in layer 2 and task bundles (`--task`, `bundle_task` 10k tokens, Phase 2); ancestors' invariants, criteria's named mutations; target bindings (`qpath`, file, **signature**, layer, `sync`) and `@verifies` tests as layers 5 and 9 (Phase 3); the `bundles` log and the follow-up-reads signal: more than three follow-up reads after a bundle are recorded in `runs` as "bundle/tree incomplete".

## 7. Proposals, owner queue and gates

The full scenario is in `06-workflows.md`. The engine invariants:

1. A proposal is created **with evidence** (`evidence`: file, symbol, lines, "what the code says", "what the spec says"), a **gap type** `gap_type: missing | partial | contradicts | unrequested` (Spec Kit converge vocabulary; `unrequested` — code nobody asked for) and **options with a price** (`options`). A `question`/`discrepancy` proposal without options is rejected: a question without options shifts the work onto the owner.
2. Before the owner sees it, a proposal is validated: the findings it introduces are attached, never refusing (shipped: `docs/canon/proposal-queue.md` "Creation").
3. **Deduplication**: creating a `question` searches FTS and the graph among `answered` questions and decisions on the same nodes. A match is returned to the agent as "already decided: DEC-…" and no question is created: re-asking a settled question is the most expensive noise in the queue.
4. `approve` calls `apply_proposal` (shipped for `update`: `docs/canon/proposal-apply.md`): applied where raised (ADR-0004, ADR-0032), always one commit `spec: apply PR-0042` with provenance (ADR-0005). To come: hashes recomputed; for deciding kinds a decision record with `cost`, an `answers` link and `canon:`.
5. **Nothing blocks** (ADR-0012): several proposals may be open on a node; on apply each rebases from its `base_hash` by three-way merge, a conflict refused with its text (shipped; resolving it in a UI diff: later). The only control point is **task approval by the owner**: `claim_task` from MCP hands out only `ready`, and `ready` is set by a human.
6. **Reconciliation with the working answer**: agent code written before the decision carries `// @assumes PR-…`. If the decision diverges from the working answer, a follow-up task is created.
7. **Task staleness**: when a node from a ready task's `spec_snapshot` changes, the task gets an informational `stale` flag. Exception — changes brought by its own proposals. The agent receives the diff before continuing.

## 8. Operational-state reliability

**SQLite.** `rusqlite` `bundled`, FTS5, PRAGMAs, `Immediate` writes, the walk, the queue's own `user_version` steps: shipped (store README, `docs/canon/proposal-queue.md`). Still to come: `PRAGMA optimize` from time to time; WAL fails on network and synced file systems, so the DB lives in the user's data directory (`crates/specengine-cli/README.md`) and the daemon warns about a repository on iCloud Desktop/Dropbox; watching by `notify` 8.2 (not 9.0-rc) + `notify-debouncer-full` 0.7 (merges atomic saves by file ID). Queue backup: `docs/features/queue-export.md`, never in git; `docs/generated/queue.md`: `task-package`.

## 9. Technology stack (versions — in `04-prior-art-and-stack.md` §6)

| Layer | Choice | Why |
|---|---|---|
| Language | Rust 2024 | consumer projects are Rust; native tree-sitter; one binary |
| HTTP | axum + tokio, SSE via `axum::response::sse` | the standard; SSE instead of WebSocket |
| MCP | official `rmcp` 3.5 (stdio + Streamable HTTP, both protocol eras) | see 04 §3–4 |
| DB | SQLite (`rusqlite` bundled, FTS5) | local, no server |
| Markdown | `pulldown-cmark` (heading attributes, offsets) | `{#ID}` sections and exact spans for patches |
| YAML | `serde-saphyr` | `serde_yaml` is deprecated, forks unmaintained since 2024 |
| AST | `tree-sitter` 0.27 + `tree-sitter-rust`; RON — own lexer | `tree-sitter-ron` 0.2.0 has no crate API for 0.27 (loads only beside a second C runtime: undefined behaviour that broke Rust parsing), rejects `#![enable(..)]` and raw strings, 5–6× slower; the lexer is parse-clean on 97.5–100 % of pilot files (the rest malformed). Resolved identity — layer C (§5.1) |
| Hash | `blake3` | fast, stable |
| Git | system `git` via `std::process` (at first), later `gix` | worktrees, diff, `merge-file`, show without bindings |
| Watch | `notify` | live drift in the daemon |
| CLI | `clap` 4 | — |
| UI | `ui/README.md` | ADR-0011, ADR-0033 |
