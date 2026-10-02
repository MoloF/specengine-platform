---
class: spec
status: shipped
scope: [crates/specengine-mcp, crates/specengine-cli]
ref: 08 §2 Phase 1 Next (MCP stdio); 07 §1.1-1.3; 04 §4; owner's answers Q1-Q6 and the show tail, 2026-10-02
shipped: 2026-10-02
---

# MCP read tools and resources

## Why

Agents read the spec via Bash or raw files; MCP is their native surface (07 §1.1, §1.3): tool search, typed arguments, read-only annotations, `@`-mentions. One core, four adapters (05 §1): MCP answers equal the CLI's (text, JSON, `bundle_hash`), write nothing under the project root and stay within a real size bound.

No ADR: Q1-Q6 decide within ADR-0001/0003, 0004/0005, 0008, 0009, 0015, 0024, 0027; MCP → CLI is canon (CLI README), never the reverse. How it works now: `docs/canon/mcp-read.md`.

## Acceptance criteria

Spawned binary, cleared env, scratch `HOME` (AC-09), scratch copies of `fixtures/spec-a`, `-b`. Parity: text == `stderr_lines` + `render_text` (exit 2: the `CliError` lines), `structuredContent` == parsed `render_json`, same request and `Env` via the CLI library. M: the mutation that must turn it red.

- [x] AC-01 Default build, both eras: `tools/list` exactly `get_context_bundle`, `get_node`, `get_tree`, `search` (stateless: `ttlMs: 0`, `cacheScope`); each the three annotations, an `outputSchema`, `MAX_RESULT_CHARS`, a description ≤ 2 048 chars holding "Deterministic: one state, one result; no LLM inside."; `instructions` ≤ 2 048 bytes, naming the four, no `review_proposal`, `probe_`. M: the demo in the default build; a 2 049-char description.
- [x] AC-02 `get_node` parity: an ID, `TERM-tired` (`aliases:`), `QST-031` (`aliases_from`), `stamina-tuning/AC-07`, `ID#SECTION`, an `.md` path, spec-b `REQ-001`; `with: ["links"]` (± `archive`) == `show --links`; a several-holders `warning:` kept. M: links re-sorted; the warning dropped.
- [x] AC-03 Parity: `search` (`kinds`, `limit` 1, 20, 200, `archive`), `get_tree` (no root, a ROOT, `depth` 0, 1, `kinds`, `archive`), each also cut at 40 000 chars (the CLI's tail, `truncated: true`). M: a cut of MCP's own.
- [x] AC-04 `get_context_bundle(["MEC-STAMINA"], 10000)`: body byte-equal; `bundle_hash` equal in text and `structuredContent`, == BLAKE3 of the body; no `budget` → `[budgets] bundle_node`, absent → 2 000. M: the body re-rendered; an MCP default budget.
- [x] AC-05 `MEC-NOPE` → `isError`, the `spec:` line, the exit-1 document. `isError`, the CLI's line, no `structuredContent`: a Cyrillic look-alike ID (Latin fix named), `project:`, `depth: -1`, only 2-char terms, `budget: 0`, below the minimum, `archive` without `with`. `depth: "x"`, an unknown argument, `with: ["bindings"]` → `isError`, no `structuredContent`, no JSON-RPC error. Each time the next call answers. M: exit 1 as success; the session ended; -32602 for a bad argument.
- [x] AC-06 Started in an empty directory: both eras start, `tools/list` answers, every tool names `spec init`, `resources/list` empty; `HOME` unset → every tool names `HOME`; `--root`, `--config` reach a project outside the cwd. M: discovery at startup.
- [x] AC-07 One session: an edited file shows in the next `get_node` without `spec index`; the DB deleted between calls → the next call answers; an edited `bundle_node` applies next; a request sent after a bundle call over ≥ 1 000 candidates is answered first (legacy `ping`, stateless `resources/templates/list`). M: a kept outcome, project or DB handle; the call on the runtime thread.
- [x] AC-08 Single door (08 AC-3), default build: a temp git repo committed clean, cwd inside; every listed tool called with valid and invalid arguments (none uncalled); every listed resource and two template URIs read; then `git status --porcelain --ignored` empty, the tree byte-identical, new files only under `HOME`. M: a tool writing under the root; a new tool without a call.
- [x] AC-09 Each spawn: cleared env, a new scratch `HOME`; after one read `<HOME>/Library/Application Support/specengine/<slug>.db` exists. M: the harness inheriting the env.
- [x] AC-10 Both eras advertise `resources`; listed: `spec://<slug>/tree`, each live document (`docs%2Ffeatures%2Fstamina-tuning.md`; no Tier 3 or `class: generated` one), the template; 250 scratch documents page 200 + 51 by cursor. Reads == the tool text for `MEC-STAMINA`, `stamina-tuning%2FAC-07`, a `%23` `ID#SECTION`, spec-b's Cyrillic legacy ID percent-encoded; a foreign slug, `MEC-NOPE` → not found per era, `data.uri`; `%ZZ` → -32602, no `data`; stateless `ttlMs: 0`. M: `{id}` undecoded; a foreign slug served; Tier 3 or generated listed.
- [x] AC-11 Two copies in opposite file orders, other roots and `HOME`s → byte-identical `result` objects for every tool and resource, none holding the root, `HOME` or a date. M: the absolute root in a header.
- [x] AC-12 A scan of the default build's sources (`src` minus the `probes`-only `review.rs`, `probes.rs`): no string literal equal to a spec-a/-b kind; no P2-3 word (07 §1.2) in a text of any source; a synthetic non-Rust corpus yields none. M: a `kinds` enum; "mechanic" in an example.
- [x] AC-13 Every input schema is flat: strings, integers, booleans, string arrays, nullable optionals, `additionalProperties: false`, no root `anyOf`/`oneOf`/`allOf`, no `$ref`, `kinds` never an `enum`. M: a nested parameter struct.
- [x] AC-14 Every `structuredContent` in the tests conforms to its `outputSchema` (types, exact key sets, all required; `omitted`, `links`, `working_answer`, `tail` once non-null), a test-local walker. M: a CLI key missing from a mirror type.
- [x] AC-15 Corpora driving each tool to the cut; control characters in titles, a document, a bundle; a links-heavy `get_node`; a document of 8 000 ID sections: text ≤ 40 000 chars + tail and notes; text + serialized `structuredContent` ≤ `MAX_RESULT_CHARS` (maxima and residue: canon "Size"). M: the declared value below the measurement; the `show` tail unbounded.
- [ ] AC-16 Deferred to the owner (canon "Owner's check", recorded: pending): `probes` build, scratch `HOME`, Claude Code version; 48 000, 60 000, 104 000, 200 000-char results ± the lever; does `structuredContent` reach the model and count; `@` autocompletion, `nextCursor`, an `@`-mention; the server's cwd; the four tools in a session here.
- [x] AC-17 `build_graph.rs` pins MCP `[dependencies]` to `clap, getrandom, rmcp, serde, serde_json, specengine-cli, tokio` (`getrandom` `probes`-optional); `rusqlite` only via the store; CLI pins, `CLI_FORBIDDEN` unchanged; no workspace dependency or tokio feature added. M: a direct store or `rusqlite` edge; a percent-encoding or JSON-schema crate.
- [x] AC-18 CLI tests green, every CLI JSON key set unchanged but `omitted` (AC-22); `INDEX_FORMAT` 6, `format_history.txt` unchanged; DB tables unchanged after every tool. M: a log table.
- [x] AC-19 `--features probes`: demo and probe tests green; `--lifecycle legacy` refuses stateless; empty stdin → exit 0; a bad first message → exit 1, one stderr line; over broken and non-UTF-8 files stdout is JSON-RPC only, stderr empty. M: a `println!` or the default panic hook on the read path.
- [x] AC-20 Gate clean; at shipping worst W ≤ 114 127 B, every Tier 1 README and the root index ≤ 10 240 B, the new canon ≤ 12 288 B, this spec < 15 737 B. M: the tool contract appended to the MCP README.
- [ ] AC-21 Met for this repository (canon "Latency": warm 19–72 ms on its docs copy; a synthetic 3 000-candidate bundle warm 886 ms, 1 821 ms at budget 10 000, so the CLI README's "lighter resolver input" is open). Deferred: cold and warm latency per tool on each pilot the owner names (read-only: `--root`, a scratch `--config`).
- [x] AC-22 CLI tests, `show` cut: a document of 8 000 `## … {#TERM-…}` sections → text ≤ 40 000 chars + a tail naming 20 sections, then `, <k> more`, k exact; JSON `sections` exactly the printed headings' IDs, `omitted.sections` the first 20 hidden at the JSON's cut (with `--links`, the tail's), `sections_more` k; > 20 holders past the cut → 20 + `, <k> more`, `holders_more`; existing uncut-output tests unchanged. M: the tail listing every section; `sections` uncut in JSON.

## Implementation

Canon: `docs/canon/mcp-read.md` (binary, tools, parity, schemas, texts, resources, "no project", state, size, latency, the owner's check, "Open"); `crates/specengine-mcp/README.md` (modules, `probes`, the Claude Code facts moved from 04 §4); the `show` cut, `documents`, `locate` in `crates/specengine-cli/README.md`; 07 §1.1–1.3, 04 §4 cut to pointers. Three iterations, each accepted by the reviewer; 916 of 916 tests, `probes` 55 of 55, clippy and fmt clean, every named mutation red.

| Module | What it does |
|---|---|
| MCP `read.rs` (new) | the four tools: argument types, descriptions and asserts against CLI constants, `MAX_RESULT_CHARS`, `call` on `spawn_blocking`, `Answer` → result |
| MCP `resources.rs` (new) | `list` (the `locate` rule, 200 per page), the template, `parse`, `read`, percent coding, codes per era |
| MCP `mirror.rs` (new) | input and output schemas (`rmcp::schemars`); mirror types of the `--json` documents |
| MCP `server.rs`, `main.rs`, `lib.rs`, `review.rs`, `Cargo.toml` | `INSTRUCTIONS`, resource handlers, cache hints; `--root`, `--config`, the quiet hook; the demo behind `probes` (+ `getrandom`); the CLI edge |
| CLI `cap.rs` | `SHOW_TAIL_NAMES`, the bounded tail, the cut node's `sections`, `omitted.*_more` |
| CLI `documents.rs` (new), `project.rs`, `lib.rs` | `documents`; `locate`, `Located` public; store constants re-exported |
| `fixtures/mcp/mcp.json` | scratch `HOME`, real `CARGO_HOME`, `RUSTUP_HOME` |

Tests: MCP `mcp_{default,read,resources,session,door,determinism,genre,index,quiet,size}.rs`, `common/{read,blake3}.rs` (new), `mcp_stdio.rs`, `common/mod.rs` (env cleared, scratch `HOME`); CLI `show_tail.rs`, `locate.rs` (new), `bounds.rs`; eval `build_graph.rs`.

Deviations from the draft, now canon: a bad argument is rmcp `Parameters`' `isError` (SEP-1303), not -32602; not found is -32002 legacy, -32602 from 2026-07-28 (rmcp's rewrite, SEP-2164), `data.uri` both; the stateless era has no `ping`; `documents` lists live files (no `class: generated`), unparsed ones too; the owner had `show`'s tail bounded here, as AC-15 found 702 503 characters for 8 000 sections (now 92 786); "no project" = CLI `locate` fails → `resources/list` `[]` (also an unusable `HOME`), any later failure -32603, config → slug → `HOME` order, so `--root nope` `[]`, `--config nope.toml` -32603 (iteration 3); `search.query` is one term; panic and parse-failure texts end without `\n`; page 1 = the tree + 199 documents. Residue (holders in plain `show` JSON, defect warnings, ID and path length): canon "Size", "Open".
