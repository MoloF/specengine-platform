---
class: spec
status: in-progress
scope: [specengine]
ref: research-2026-09-28
---

# 04. Prior art, the MCP protocol, Claude Code, the stack

> External research as of 2026-09-28. ⚠ — not confirmed against a primary source.
> One-line conclusion: **no ready product exists**. Well-covered pieces: SDD processes, AST fingerprints, code graphs, task trackers for agents,
> and tracey (Rust) already implements the "daemon + CLI/MCP/LSP bridges" topology and requirement versions in markers.
> Nobody covers: **a spec graph with identity and revisions + binding to symbols + an approval queue**. That is SpecEngine.

## 1. Prior art: what to take, why not use it as is

### 1.1. Spec-driven development

| Tool | Essence | Take | Why not it |
|---|---|---|---|
| **GitHub Spec Kit** (v1.0.1, Aug 2026) | `specify → plan → tasks → implement`, markdown artefacts per feature + project **constitution** | the constitution idea (project-level invariants above features); separate `/clarify` and `/analyze` phases | folders of prose per feature; no node identity, graph, code binding, drift or approval |
| **AWS Kiro** | `requirements.md` in **EARS** notation ("WHEN \<trigger\> THE SYSTEM SHALL \<behaviour\>") + design + tasks; steering files; on-save hooks | **EARS as the recommended form of a leaf rule** (atomic, testable, diffable) | closed IDE, paid, no drift or graph |
| **OpenSpec** (Fission-AI) | **delta specs**: a change folder (proposal/specs/design/tasks) → archiving merges into the living spec | the delta model for brownfield — exactly our "agent proposes a spec edit" | changes are files, not reviewed records; no code binding |
| **BMAD** v6.8 | 7–12 roles, 4 phases | role separation (already in place) | most token-expensive; a process, not a data model |
| **Agent OS v3** (Jan 2026) | `/shape-spec`, `/inject-standards` | signal: v3 **removed** implementation-orchestration phases — models cope on their own. **Build a spec store, not an orchestrator** | standards injection only |
| **Task Master** (`task-master-ai`, 36 MCP tools) | PRD → tasks; `next_task`, `expand_task` | **tool-set levels** (core/standard/all) to save context; the `next_task` ergonomics | a task tracker, not a spec tree |
| **Beads** (`bd`) — closest relative | task graph for agents: `blocks/parent-child/supersedes/duplicates`, hash IDs, `bd ready` (no open blockers), compaction of closed items, CLI+MCP | hash IDs against merge collisions; **`ready` query over the blocker graph**; compaction; CLI/MCP parity | **storage lesson**: moved from JSONL-in-git to Dolt, users complain about sync between worktrees (issues #3135, #4074). See §5 |
| **HumanLayer / CodeLayer** | daemon + Tauri UI + CLI around Claude Code with deterministic approval gates; ACE/FCA method (research → plan → implement, context load 40–60 %) | the product shape "daemon + UI + gates"; the argument "one bad plan line = hundreds of bad code lines" | approves agent actions, not specs; no business-logic graph |

**Data on SDD benefit** (vendor benchmark of 2026-09-24, 50 tickets, ⚠ dates inside the report are inconsistent): merge rate 80–84 % with specs vs 72 % without; defects per ticket 0.46 vs 0.86. **The gain is concentrated in features and changes touching 3+ files; on bug fixes, refactors and small edits there is none**, while the spec phase eats 28–44 % of tokens. Conclusion: **gates must be selective** by path and change size from day one.

### 1.2. Requirements traceability

- **OpenFastTrace**: revision in the ID (`dsn~name~1`), in code `[impl->dsn~name~1]`. Bumping to `~2` makes coverage `outdated`. The cleanest published answer to spec drift. We get the same automatically through `spec_hash` in the lock (03 §2.5).
- **Doorstop**: a YAML file per item; the link to the parent stores a fingerprint of the reviewed version. If the parent changed, the link becomes **suspect** until re-review. Matches our model (⚠ exact field names unverified).
- **StrictDoc** (`.sdoc`, local web server, `@relation` markers in code ⚠) — the closest thing to "SpecEngine without agents".
- Sphinx-Needs, ReqIF, DOORS/Polarion — the ceremony does not pay off for a solo developer. Export to ReqIF/`needs.json` — "maybe" at most.

### 1.3. Code context and Rust symbol identity

| Tool | What it gives | Verdict |
|---|---|---|
| **Aider repo-map** | **file** graph + ranking, signatures, 1,000 tokens by default | wrong granularity for bindings, but "signatures instead of bodies" is ours |
| **Serena MCP** | LSP (rust-analyzer): `find_symbol`, `find_referencing_symbols`, `replace_symbol_body` | a good **complement** for agent navigation; knows nothing of specs or bindings |
| **`rust-analyzer scip`** | resolved identity: `rust-analyzer cargo <crate> <ver> <mod>/<fn>().` | **enrichment option** for stable paths (limits: nested definitions #18771, inherent-impl ambiguity #18772); ⚠ time on a large project unmeasured. Call the CLI, do not link `ra_ap_*` |
| **code-graph-mcp** (v0.132) | tree-sitter for 19 languages with full Rust, SQLite + FTS5 + sqlite-vec, **BLAKE3 Merkle per file** for incremental reindexing, call graphs, impact analysis, **detail levels L0–L3 and `compact: true`**; ~139 files/s, ~4.2 MB of DB per 1,000 nodes | **read its schema before designing ours**: it is essentially the "code half" of SpecEngine. At hundreds of kLOC expect an index in tens of seconds and tens of MB |
| **ast-grep** | relational patterns (`inside`/`has`), `ast_grep_core` as a library | handy for `add_systems`/`add_observer` patterns |

### 1.4. Documentation drift — the main find

**fiberplane/drift** (blog 2026-03-25, "a linter for documentation rot") covers about 60 % of our drift engine:

- binding `drift link docs/auth.md src/auth/provider.ts#AuthConfig`, inline `@./src/...#Symbol`;
- **fingerprint = XxHash3 of the normalised AST (node kinds + token text, no whitespace or positions)**, grammars include **Rust**;
- bindings live in **`drift.lock` (TOML)** with a git SHA and a base via `git show`;
- `drift check [--changed path]` (exit 1), `status`, `refs`, GitHub Action, installs as a Claude skill;
- admitted gaps: nothing stops re-binding without editing the prose; rename and move are unhandled; no nodes, graph or approval.

Our `spec.lock` (05 §3.5) independently arrived at the same shape, which confirms the design. SpecEngine differs: the marker sits in the **code** and moves with the symbol, the binding is to a **spec node**, not a document file, and there is an approval queue. ⚠ drift is written in **Zig** (v0.10.1): not usable as a library, only as a design and CLI reference.

Others: `spec-drift-mcp` (TypeScript, checks fields only, inactive). **Swimm left the doc-drift market.** The niche is open.

### 1.5. tracey (bearcove) — the closest Rust analogue  ⟵ read the code before Phase 1

`github.com/bearcove/tracey`, MIT/Apache-2.0, crates.io 1.3.0 (491 downloads; README says install from `main`, single maintainer). README verified 2026-09-28:

- **Topology matches ours**: a persistent daemon per workspace; web dashboard, LSP, MCP (stdio) and CLI are bridges to the daemon over the Unix socket `.tracey/daemon.sock`. The daemon watches the FS (debounced), every bridge starts it itself, it exits after 10 minutes idle. **We take this scheme**: `spec mcp` and hooks bring the daemon up automatically.
- **Requirement version in the marker**: in the spec `r[auth.login+3]`, in code `// r[impl auth.login+3]`. After a bump to `+4` the `+3` reference becomes **stale** and stops counting as coverage. `tracey pre-commit` **fails if a rule's text changed without a version bump**; `tracey bump` bumps versions of changed rules and re-adds them to the index. Discipline: first the code is brought to the new text, **then** the marker is bumped; the bump is the review record.
- LSP hover shows a **word-level diff** of the rule text between the marker's version and the current one. Exactly what a human or agent needs for re-approval.
- MCP responses are **prose with a status header, a "Delta — what changed since the last query" section and "Hints — what to ask next"**. Cheap and convenient for an agent; we take it.
- `tracey query unmapped` shows the source tree with coverage percentages, i.e. **where there is no spec**. The flip side of `uncovered`; take it into the MVP.
- One spec can have several `impls` — useful for invariants shared by several projects.

**Why not take tracey wholesale**: flat namespace (dots in IDs give no tree, no parent/child or dependency graph), no node types, no approval. Binding is to the **comment location**, not a resolved symbol; there is no code drift (only spec versions). The "own vs layer over tracey" decision: ADR-0019.

### 1.6. More analogues and vocabularies worth taking

- **amiss** (Rust, 0.36.0 of 2026-09-26, FSL-1.1 → Apache-2.0 after 2 years) — "documentation against the repository tree". Taken by `spec check` (`docs/canon/spec-check*.md`): exit 0 / 1 / 2 (the run cannot be trusted, no verdict), "could not check" never "fresh", expiring debt instead of an ignore file, observe → enforce-introduced → enforce. Still to take (Phase 3): the verdicts Fix (this change broke it) / Check (a human should re-read) / Pre-existing (backlog); drift is "the file changed under a paragraph that did not", both changing at once is not.
- **Doorstop** is alive (v3.2 of 2026-07-10, `doorstop-dev`). The `reviewed:` fingerprint is computed over UID + text + links and **deliberately excludes presentation fields** (`active`, `level`, `header`…). A link is stored as the pair "parent UID + parent fingerprint at review time". Take both.
- **StrictDoc** (v0.30.1 of 2026-09-16) already binds to **canonical Rust paths** (`<file::FooTuple as file::FooTrait>::bar`) and `@relation(REQ-N)` markers in doc comments. Scope vocabulary `file/class/function/range/line` + `@relation(skip)` as an explicit blind spot. Crate `strictdoc-parser` 0.1.1 exists. In StrictDoc's own backlog: "requirement checksum calculation postponed" — our key function is an admitted gap there.
- **OpenFastTrace** 4.10: full coverage-status vocabulary — covers / **predated** (code references a **newer** revision than exists) / outdated / ambiguous / unwanted / orphaned; coverage requirement **on the node** (`Needs: impl, utest`), not global.
- **Sphinx-Needs** 8.5: link types are user-defined (`needs_extra_links`) — do not hard-wire the link vocabulary; declarative graph invariants ("every invariant has ≥ 1 `verifies`").
- **Spec Kit `/speckit.converge`**: four mutually exclusive gap types — **missing | partial | contradicts | unrequested** (code nobody asked for), severity, an evidence table and the rule "a claim of completion is not evidence". Its stated non-goal ("not a diff tool, no git or history") is our wedge: we do the same check **incrementally**. Take it as `gap_type` on a discrepancy.
- **Reqvire** (Rust, ⚠ not found on crates.io): `change-impact` = git diff + link-graph traversal. Lesson from their issue #73: an MCP server without a persistent index re-read the model on every call, **17–80 s per read**.
- **Demand confirmed**: Kiro issue #9435 "Record git ref in spec metadata to detect code drift" (2026-06-15, open, unanswered).

**Positioning** (Böckeler, martinfowler.com, 2025-10-15): spec-first / **spec-anchored** / spec-as-source. SpecEngine is **spec-anchored**: the spec lives, a feature evolves through it, code is not generated from the spec. Tessl, the most radical spec-as-source vendor, reportedly pivoted after about 4 months (⚠).

**Academic anchors** (⚠ per the research, unverified): arXiv 2606.27045 "The Spec Growth Engine" — the closest published architecture (spec graph, context assembler along the ownership path, **drift gates as a blocking merge condition**); 2605.17246 "Fidelity Probes" — questions generated from code and answered from the spec yield a subtree health metric; 2609.18298 — the auditor works in a **clean context**, separate from the code author. Strongest counter-argument (Willison): **agents get their power from the test suite, not prose** — hence evidence levels (05 §5.5).

### 1.7. Rust symbol identity — what works and what does not

| Approach | Verdict |
|---|---|
| **`ra_ap_ide::StaticIndex::compute`** (rust-analyzer as a library, `ra_ap_*` 0.0.352 of 2026-09-14) | **best source of resolved identity**: one pass yields `moniker`, `signature`, `definition` (name span) and **`definition_body`** (span of the whole item, for the AST hash). Cost: no stability guarantees, weekly releases, **pin `=0.0.352` for all `ra_ap_*` at once**; static loading, incrementality is ours. Memory and time: type inference (60 of 90 s on rust-analyzer itself) is **not needed** for monikers (item tree + def map suffice); on the Bevy pilots, through `MonikerResult::from_def`, measured affordable on a laptop (05 §5.1 layer C). Without a proc-macro server `#[derive(Component)]` and other derives are lost |
| `rust-analyzer scip` (subprocess) | weaker: **single-threaded** (issue #18140, mozilla-central about 20 min), no item bodies. But we take the **SCIP symbol grammar** as the on-disk ID format |
| LSP client to rust-analyzer | ⚠ **trap**: LSP has no `textDocument/moniker`, `documentSymbol` returns text as written (`"impl fmt::Debug for E"`). Building identity via LSP is a dead end |
| `syn` 3.0 / tree-sitter | do not resolve names: an `impl` has no resolved self type or trait, adjacent impls are indistinguishable. tree-sitter is fine **for hashing and markers** (05 §5.2), neither for identity |
| rustdoc JSON | nightly only, format broke 4 times in 5 weeks, **`Id`s unstable**, impls unnamed, no bodies. A secondary source at most |
| stack-graphs | archived 2025-09-09, never supported Rust |
| Aider repo-map | **blind to Bevy registration**: `add_systems(Update, move_player)` matches no capture in `rust-tags.scm`, so a system that is only registered gets near-zero rank. The upstream tags query knows no `impl_item`, const/static, variants or fields |
| Serena | **GPL-3.0**: ideas may be taken, code may not. Identity is the LSP symbol's file path, cache invalidated per whole file. Worth taking is the memory design: "list names first, read deliberately", "references beat search" |

**SCIP grammar** (real rust-analyzer strings): `rust-analyzer cargo foo 0.1.0 example_mod/func().`, trait-impl method `…module/impl#[MyStruct][MyTrait]func().`. Two mandatory changes: **replace the package version with `.`** (otherwise a version bump in Cargo.toml rewrites every ID) and **add a `disambiguator` for adjacent inherent impls** (issue #18772: two `impl Foo {}` in one module collide, the duplicate is silently dropped). Since 2026-03-25 SCIP lives in an independent project with open governance (scip-code.org); crate `scip` 0.10.0 reads the index natively. Meta Glean indexes Rust precisely via `rust-analyzer scip`.

**Bevy system registration — by resolved types, not text.** The reference is the `restriction::schedule` lint from `bevy_lint` (TheBevyFlock): the `add_systems` method is recognised by the **resolved receiver type** (`App`), the schedule by the resolved type of the first argument. The same is done in rust-analyzer HIR (`Semantics::resolve_method_call`/`resolve_path`), and then systems land in **the same ID space** as everything else. Must traverse tuples and `.chain()/.after()/.in_set()/.run_if()`, `impl Plugin::build` bodies (including in dependencies), `add_observer`, `register_system*`. Registration via `inventory`/`linkme` is statically invisible — report it as a blind spot. Registrations **inside `macro_rules`** do occur; expand them. Scale: tens to hundreds of `add_systems` per project. **Test oracle** — `bevy_mod_debugdump` 0.16 (`--dump-schedule` → DOT with ordering edges). Full `type_name` paths at runtime need the `bevy/debug` feature.

**ast-grep** (`ast-grep-core` 0.45.3, verified by the research on Bevy code): the pattern `$APP.add_systems($SCHED, $$$SYS)` finds every registration, chains included, but returns **expression text, not identity**. Suitable as a **user DSL for anchors** ("the node is bound to whatever matches this rule", vocabulary `inside/has/stopBy`) paired with a resolved moniker. `ast-grep outline` as a symbol source **is not suitable**: it loses nested modules and impls. The `sg` command is deprecated since 0.45.0; use `ast-grep`.

### 1.8. Agent-memory anchors on AST hashes: limpet, sem, mago

- **limpet** 0.17.0 (2026-09-08, Rust, tree-sitter + rusqlite + MCP, single binary, no network; 419 downloads): "memory for agents that goes stale with the code". Every memory record is anchored to a normalised AST hash. Rename or move — the record follows the code; edit — the record visibly goes stale with a reason; revert — the record "recovers". **The core of our drift is already written and audited**, so read limpet's code before designing. Techniques to take: **the symbol name is not part of the hash**, an **entropy threshold** against false-following of trivial twins, a discriminator for colliding paths, deterministic resolution independent of row order (their 2026-07 audit: anchors "flickered" because of `LIMIT 1`). Unlike SpecEngine, limpet has no spec tree or approval.
- **code-graph-rag (cgr)** (Python, MIT, ~5.2 k stars, v0.0.996 of 2026-09-26; docs verified): the `glosses` tool — notes anchored to symbols, **with a graded anchor state `EXACT / STALE / MOVED / AMBIGUOUS / LOST`**. Take the vocabulary verbatim (05 §5.3). Also worth taking: three fingerprints (skeleton, per-branch, text quote with the name masked), rename detection by skeleton pair, the **write-response contract** (`symbols: {added, removed, renamed, changed}`, `dangling_callers`, `tests_reaching`, …) and `cgr check --base <ref>`, a **suffix index** (48 % CPU → 178–382× speedup), resolution confidence on every link and refusal of destructive operations on heuristics, a split into deterministic tools (default) and LLM tools (fallback). Real reindex measurements: a module with two dependants — p50 194 ms, a 54-file hub — p50 3.5 s. Do **not** take: Memgraph + Docker + Qdrant, NL→Cypher, the LLM layer. cgr's Rust is tree-sitter heuristics only. **No studied tool sees Bevy system registration** (verified in cgr's sources) — that remains our differentiator.
- **The Rust cluster "tree-sitter + SQLite + MCP without LLM"** (small projects: read the data models, do not depend): `sunerpy/codegraph-rust` ("deterministic code graph for agents, byte-stable output", `docs/data-model.md` — read before designing the schema); `dertin/code-system-graph` — a crate-split reference (`-core`, `-model`, `-store-sqlite`, `-hooks`), atomic SQLite snapshots and stable node IDs.
- **sem** (`sem-core` 0.25.0, 46 k downloads): "semantic version control on top of git" — which **entities** changed, not lines; 28 languages, git merge driver and MCP server. A candidate for explaining drift ("what changed in the symbol").
- **mago-fingerprint** 1.50.0: three rules — **length prefix** on every byte string (`"ab"‖"c"` ≠ `"a"‖"bc"`), a **fixed-seed hasher** (a `RandomState` digest is useless in storage), an explicit policy for significant comments.
- **normalize-code-similarity** 0.3.2: MinHash LSH and normalised AST hash as a library; a fallback for fuzzy move search.
- **difftastic is not a library** (since 0.12). For **explaining** drift after a hash fires, `syndiff` 0.2.0 (structural diff in the difftastic spirit) fits. similarity-rs `SubtreeFingerprint` (weight + child hashes) is a good prefilter for "where did the body move".
- Origins: Baxter et al., "Clone detection using abstract syntax trees", ICSM 1998 — hashing AST subtrees; clone taxonomy Type-1…4 (Roy/Cordy/Koschke 2009). Our hash catches changes beyond Type-1 (everything except formatting and comments) with the symbol name excluded.
- **Why not the git hash**: the blob OID is computed over bytes and changes on every `cargo fmt`. Git does not know what a function is — that is SpecEngine's motivation in one line. The OID serves only as the cheap first step of the cascade ("the file did not change at all").

**Claude Code already does symbol navigation**: the official code-intelligence plugin for Rust gives the agent an `LSP` tool (definition, references, types, file and workspace symbols) that works "by symbol, not text search". So SpecEngine **does not duplicate navigation** and owns only the "spec node ↔ symbol" link. Consumers should install it independently of SpecEngine.

## 2. Evidence base on context

- **Context rot is measured** (Chroma, 18 models): quality drops with input length for every model; a 200 k window can degrade noticeably already at 50 k.
- **Anthropic, "Effective context engineering" (2025-09-29)**: "the smallest set of high-signal tokens"; **just-in-time** loading by lightweight identifiers instead of preloading; do not bloat the tool set; sub-agents return 1–2 k-token digests.
- **Anthropic, "Effective harnesses for long-running agents" (2025-11-26)**: one feature at a time; **JSON, not Markdown, for machine-driven lists**; git as a checkpoint. Human gates are not described there — that is our difference.
- **Anthropic, "Writing effective tools for agents"**: search instead of list-all; domain prefixes; responses ≤ ~25 k tokens with pagination; at ≥ 10 tools definitions are deferred (Tool Search).
- **Agent Skills**: discovery costs ~80 tokens per skill, the body loads on demand (progressive disclosure).
- **CLAUDE.md**: consensus — up to ~200 lines; "CLAUDE.md as RAM, skills and sub-agents as disk". Since v2.1.277 Claude Code reads `AGENTS.md` when `CLAUDE.md` is absent.

For SpecEngine this means: the context bundle is identifiers plus minimal text, follow-up reads by id, digests instead of dumps, a small tool set with good `instructions`.

### 2.1. Vector search: not needed (⚠ research data, primary sources not opened)

- **Cursor** published the best pro-embedding data in November 2025: +12.5 % accuracy offline. In production the effect was modest: code retention +0.3 %, +2.6 % on repositories of 1,000+ files. **Around July 2026 Cursor switched semantic indexing off in favour of a grep index** ("models got very good at using grep"); current docs say it "does not store codebase embeddings for search".
- **GitHub Copilot**: +37.6 % retrieval quality from a new embedding model became "2 % less time with no change in quality" for the agent.
- **CORE-Bench** (arXiv 2606.11864): the embedder is good at "find similar code" (71.7) and **fails at "find the code this task is about"** (20.3). Spec → code binding is the second task.
- **In favour of our artefact** (arXiv 2607.11046): role descriptions of code give up to +40 % Hit@5 over file paths at 10–21× smaller size than the sources. **A spec node bound to a symbol is exactly such a role description**, written by a human.
- The 2026 tool consensus is deterministic local search: lexical plus structural. Sourcegraph: "deterministic structure where it exists, semantic search where it does not". Anthropic's well-known "grep beats RAG" line is not backed by a published measurement and must not be cited as data.

**Conclusion for SpecEngine**: FTS5 + graph + resolved symbols. No vector index, none planned. Question deduplication — FTS5 over text plus node overlap in the graph.

## 3. MCP as of 2026-09-28

**Current revision is `2026-07-28`**, breaking and **stateless**. Revision history: 2024-11-05 → 2025-03-26 → 2025-06-18 → 2025-11-25 → 2026-07-28.

| Change | What it means for SpecEngine |
|---|---|
| No sessions, `Mcp-Session-Id` or `initialize`; every request carries `_meta` with client version and capabilities | state between calls = **explicit handles in arguments** (`proposal_id`, `task_id`), which is our design anyway |
| `server/discover` is mandatory | rmcp does it |
| `resources/subscribe` → `subscriptions/listen` (one long stream) | live notifications "proposal resolved", "node changed" |
| **Server-initiated requests removed**: elicitation, sampling and roots go through **MRTR** — the server replies `resultType: "input_required"` + `requestState`, the client gathers input and **repeats the request** | approval right in the session: the tool returns `input_required`, Claude Code shows a form or URL to the human. `requestState` is HMAC-signed (rmcp has the `request-state` feature) |
| **Tasks** moved to the `io.modelcontextprotocol/tasks` extension (`working/input_required/completed/…`); the docs name "approval gates" as a scenario | semantically ideal, but **Claude Code does not support it** → plan for the future |
| Deprecated (12-month window): Roots, **Sampling**, Logging, OAuth DCR, **HTTP+SSE** | do not build on sampling or the SSE transport |
| `outputSchema` — any JSON Schema 2020-12; `ttlMs` and `cacheScope` mandatory on list/read; `readOnlyHint`/`destructiveHint` annotations | mark every reading tool `readOnlyHint` |
| Skills over MCP (`io.modelcontextprotocol/skills`, Final) | Claude Code is absent from the client matrix → do not count on it |

## 4. Claude Code: what can be used (v2.1.283)

**MCP client** (docs: code.claude.com/docs/en/mcp; the era, elicitation, output-cap and background rows are verified on 2.1.283 by script and by hand — re-verify on upgrade):
- transports `stdio | http | ws` (`sse` deprecated); project scope — **`.mcp.json` in the repository** (read at session start);
- **resources via `@server:uri`**: `@specengine:spec://node/R-12` — ⚠ content is inserted **without a tool call**, so `PreToolUse` hooks do not fire;
- **MCP prompts = slash commands**: `/specengine:prepare-task T-0107`;
- **elicitation is supported** (since 2.1.76, form and URL) **in both protocol eras**; with permission prompts bypassed the form still appears, so bypass mode cannot skip it. The form is a flat object of primitives and enums; answers `accept | decline | cancel`; URL ≤ ~8,000 characters;
- sampling — no data on support; the Tasks extension — unsupported;
- a stdio server gets the **legacy handshake** (`2025-11-25`) by default; `MCP_PROTOCOL_NEGOTIATION=auto` negotiates 2026-07-28 → **support both eras**. rmcp has no server-side lifecycle mode (`ClientLifecycleMode::Auto` is client-only): the server detects the era from the first message (`initialize` → legacy; a request with complete 2026-07-28 `_meta` → stateless) and bounds it with `supported_protocol_versions`;
- **the output cap counts characters**: 48,000 pass inline with no warning reaching the model, 104,000 are rejected, and `MAX_MCP_OUTPUT_TOKENS` does not raise it. **Tool descriptions and server `instructions` are truncated at 2,048 characters**. Tool search is on by default, so `instructions` is the server's most important text;
- **an MCP call longer than 120 s goes to the background**, runs to completion and returns its result as a notification; a call held by an open elicitation form is exempt (≥ 11 min observed, no timeout). **Do not design a long-blocking "wait for approval"**;
- ⭐ **`_meta["anthropic/requiresUserInteraction"]: true`** on a tool (≥ 2.1.199): a permission prompt **on every call, even in `bypassPermissions`**, no "don't ask again", allow rules ignored, a `PreToolUse` hook cannot approve it. **The strongest primitive of human consent.**

**Hooks** (~33 events): `PreToolUse`, `PostToolUse`, `PermissionRequest`, `UserPromptSubmit`, `Stop`, `SubagentStart/Stop`, `SessionStart`, `WorktreeCreate/Remove`, `FileChanged`, **`Elicitation`/`ElicitationResult`**, `TaskCreated/Completed` and more. Handler types: `command`, **`http`**, `mcp_tool`, `prompt`, `agent`. Exit 2 blocks; priority `deny > defer > ask > allow`; `if: "Edit(src/**)"` narrows the trigger.

⚠ **An HTTP hook cannot block via a response code**: non-2xx, unavailability and timeout count as non-blocking errors, and the edit goes through. If SpecEngine is down, the gate is **open**. Fail-closed needs a `command` hook (curl + `exit 2` on unavailability) and a deny rule in managed settings as a backstop. Cover `Bash` too: an agent can write via `cat > file`.

**Delivery — a Claude Code plugin** (`.claude-plugin/plugin.json`): MCP server + hooks + skills + sub-agents in one versioned install. Cost: the plugin's skill and agent names are visible in the context of every turn (`/plugin` → Context cost).

## 5. Storage: git vs DB — checked against the Beads experience

Web research recommends the opposite of our decision: SQLite as the truth, export to git. Check:

- Beads suffered on **tasks** — high-frequency writes from many writers in different worktrees. That state lives **in SQLite** for us anyway (queue, tasks, runs).
- Specs are low-frequency writes, edited by a human, reviewed, and must live **on the branch with the code**. Consumer projects already work this way: "no second store for documents".

Result: the split by write frequency and owner (05 §1) is the Beads lesson, not its violation (ADR-0001).

## 6. Stack (crates.io versions as of 2026-09-28)

A version with `=` is an exact pin of the root `Cargo.toml` `[workspace.dependencies]`, approved by the owner and in use; the rest are planned and pinned on adoption. A new dependency or a version change is an owner decision. **Toolchain**: edition 2024, `rust-version = 1.90`; `specengine-ra` needs rustc ≥ 1.98 (the `ra_ap_*` set); no `rust-toolchain.toml`.

| Crate | Version | Role / note |
|---|---|---|
| `rmcp` | **=3.5.0** (1.0 → 3.5 in 7 months) | official SDK: `#[tool]`/`#[tool_router]`/`#[prompt]`, stdio, **Streamable HTTP as a Tower service → mounts in axum**, MRTR, elicitation, `request-state`, Tasks (`TaskManager`), `subscriptions/listen`; supports 2026-07-28 and older revisions. Features in use: `server`, `macros`, `transport-io`, `request-state` — stdio only, no HTTP stack in the graph; the `elicitation` feature is skipped (it pulls `url`), `elicitation/create` goes through `send_request`. ⚠ check the SDK tier |
| `tokio` / `getrandom` | =1.53.1 (`rt`, `macros`, `io-std`, `time`) / =0.4.3 | MCP server runtime / per-process `requestState` HMAC key; both already in the graph through `rmcp` |
| `axum` | 0.8.9 | HTTP, SSE; the next release is a breaking **0.9**, plan the migration |
| `tree-sitter` | =0.27.0 | Rust ≥ 1.90 |
| `tree-sitter-rust` | =0.24.2 | **compatible with 0.27** (ABI 15); `has_error()` check per item (05 §5.2). In 0.26+ `set_timeout_micros` is removed, cancellation via `ParseOptions { progress_callback }`; in 0.27 `child_count()` → `u32`, `kind()` → `&'tree str` |
| own RON lexer | — (`specengine-code`) | `.ron` markers, field paths, the Bevy dump reader; replaces `tree-sitter-ron` 0.2.0 (05 §9) |
| `rusqlite` | **=0.40.2** (`bundled`: SQLite 3.53.2, FTS5) | the index; not `sqlx`. Transitive, pending the owner (Q1, store README): `libsqlite3-sys` 0.38.2, `fallible-iterator` 0.3.0, `fallible-streaming-iterator` 0.1.9; build: `pkg-config` 0.3.34, `vcpkg` 0.2.15 (unused) |
| `pulldown-cmark` | =0.13.4 (no default features) | heading attributes `{#ID}`, offset iterator for precise patches |
| `serde-saphyr` | =1.3.0 (`deserialize` only) | YAML front-matter (why: 05 §9). `deserialize` pulls unpinned crates no feature turns off: granit-parser, arraydeque, annotate-snippets (+ anstyle, unicode-width), encoding_rs_io, encoding_rs (+ simdutf8, multiversion_no_op; core_detect on x86) — pending the owner (Q1, core README) |
| `serde` / `serde_json` | =1.0.229 (`derive`) / =1.0.151 | serialization; JSON output of tools and `specengine-eval`; `float_roundtrip` in `specengine-store` |
| `toml` / `regex` | =1.1.4 (`std`, `parse`, `serde`) / =1.13.1 (`std`, `unicode`) | configs (`specengine.toml`, importer) / importer ID patterns; `regex` is already in the graph through tree-sitter |
| `blake3` | =1.8.7 | node and AST hashes |
| `notify` + `notify-debouncer-full` | 8.2.0 + 0.7.0 | file watching; **not** 9.0-rc; debouncer-full coalesces atomic saves |
| `rusqlite_migration` | 2.6.0 | schema migrations (`refinery` and `sqlx` conflict with rusqlite 0.40 over `libsqlite3-sys`) |
| `similar` | 3.2.0 | proposal diffs |
| `petgraph` | =0.8.3 (no default features) | `depends-cycle` (`tarjan_scc`; owner, Q-C); new to the default members: `fixedbitset` 0.5.7, `hashbrown` 0.15.5, `foldhash` 0.1.5 |
| `gix` | 0.88.0 | later; system `git` at the start |
| `rust-embed` | 8.12.0 | UI inside the binary |
| `schemars` | 1.2.2 | JSON Schema 2020-12 for tools, through the `rmcp::schemars` re-export |
| `clap` | =4.6.7 (`derive`) | CLI |
| `ra_ap_*` (nine direct: `load-cargo`, `project_model`, `ide`, `ide_db`, `hir_expand`, `vfs`, `paths`, `syntax`, `proc_macro_api`) | **=0.0.352** (2026-09-14), all together (`ra_ap_edition` is already 0.0.354 — the set is skewed) | `MonikerResult::from_def` for resolved identity (layer C); only in `specengine-ra`, outside `default-members`, so none enters the core graph; needs rustc ≥ 1.98; API without guarantees; cargo-modules lags head by ~7 weeks — a realistic update pace |
| `salsa`, `salsa-macros`, `salsa-macro-rules` / `unicode-ident` | =0.28.2 / =1.0.24 | direct exact pins of `specengine-ra`: the `ra_ap` manifests ask `^`, and newer versions break the 0.0.352 build (salsa 0.28.5 changed `HashEqLike`; unicode-ident 1.0.26 is Unicode 18 against `unicode-properties` 0.1.4's 17). One lock file, so the `unicode-ident` pin governs the core graph too |
| `libc` | =0.2.189 | `specengine-eval` feature `ra` only: peak RSS and process groups |
| `scip` | 0.10.0 | symbol format, index reading |
| `ast-grep-core` | 0.45.3 (pin exactly) | anchor DSL for user binding rules |
| `bevy_mod_debugdump` | 0.16.0 | schedule fallback: `print_schedule_graph(&mut app, Update)`, needs only a built `App`; not needed on the pilots |
| (Bevy) `bevy_dev_tools::schedule_data` | in 0.19 | **built-in dump of all schedules in RON** (features `debug` + `schedule_data`); the truth for system registrations (05 §5.1, layer B) |
| `syn` / `quote` / `proc-macro2` | =3.0.6 (+ `extra-traits`) / =1.0.47 / =1.0.107 | comparison only, behind `specengine-eval`'s `syn` feature: not an identity source (no name resolution) and not the digest (05 §5.2, ADR-0021) |
| `limpet`, `sem-core`, `mago-fingerprint` | 0.17.0 / 0.25.0 / 1.50.0 | **read the sources**: anchors, rename following, fingerprint rules |
| `syndiff` | 0.2.0 | structural diff for explaining drift in the UI |
| `bevy` | 0.19.1; 0.20.0-rc.1 released 2026-09-15 | target of the system detector; ⚠ the 0.20 schedule API may affect it |

CRDTs (`loro` 1.16, `automerge` 0.12, `yrs` 0.28) are **not needed**: one human writer and N proposing agents is a review-queue problem, not collaborative editing. Optimistic locking by `base_hash` + SSE is enough.
