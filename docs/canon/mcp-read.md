---
class: canon
tier: 2
scope: [crates/specengine-mcp, crates/specengine-cli]
owner: owner
reviewed: 2026-10-07
---

# MCP read tools and resources

The MCP stdio pass (`docs/features/mcp-read.md`, 07 §1.1–1.3): agents read the spec through their native surface (typed arguments, read-only annotations, tool search, `@`-mentions) and get exactly what the CLI answers. One core, four adapters (05 §1): every tool and resource is one call into the CLI library (`crates/specengine-cli/README.md`), so discovery, freshness, REF resolution, the cut, tails, hints, exits and writes are the CLI's; MCP adds no cap, sort, default or rendering. Eras, stdout, exits, the Claude Code facts relied on, the Phase 0 demo: `crates/specengine-mcp/README.md`.

## Binary

`specengine-mcp [--lifecycle auto|legacy] [--root DIR] [--config FILE]`: `--root`, `--config` are the CLI globals (`Globals`) of every read; nothing is discovered at startup. The default build serves the four read tools, the four queue tools (`agent-intake.md`), the five task tools (`task-package.md` "MCP") and the resources; feature `probes` (never default) adds the Phase 0 demo `review_proposal` (with `getrandom`), `probe_output`, `probe_sleep` and a paragraph of `instructions`. `spec mcp`, the daemon relay, is Phase 2. Launcher: `fixtures/mcp/mcp.json` (`probes` build, `HOME` = `${TMPDIR:-/tmp}/specengine-mcp-home`).

## Tools

| Tool (types as the flags) | ≙ `spec` |
|---|---|
| `get_tree {root?, depth?, kinds?, archive?}` | `tree [ROOT] [--depth N] [--kind K]… [--archive]` |
| `get_node {id, with?: ["links"], archive?}` | `show REF [--links] [--archive]`; `span_hash` names a proposal's base |
| `search {query, kinds?, limit?, archive?}` | `search "QUERY" [--kind K]… [--limit N] [--archive]`, `query` one term |
| `get_context_bundle {node_ids: [REF…], budget?}` | `bundle REF… [--budget N]` |

**Parity.** `content` = one text block, byte-equal to `spec <…> 2>&1`: `Outcome::stderr_lines` (each + `\n`), then `render_text`. `structuredContent` = the `--json` document (`render_json`, parsed). Exit 0 → success, zero hits too; exit 1 → `isError`, the text and the exit-1 document; exit 2 → `isError`, the `CliError` line(s) + `\n`, no `structuredContent`. An argument the input schema refuses (a wrong type, an unknown name, a value outside `with`'s enum) is rmcp `Parameters`' error result before any call: `isError`, `failed to deserialize parameters: …`, no `structuredContent`, never a JSON-RPC error (SEP-1303: the model corrects itself). A panic in the call → `isError`, `internal error: <tool> failed` (no `\n`); a `--json` that does not parse (never expected) → `internal error: <tool> failed: its --json document does not parse: <e>`. No error ends the session. Determinism is the CLI's: no result holds the root, `HOME` or a date.

**Annotations** of the reads exactly `readOnlyHint: true`, `destructiveHint: false`, `openWorldHint: false`; `_meta {"anthropic/maxResultSizeChars": 500000}` (Size).

**Schemas.** Input from the argument types (`rmcp::schemars`, 2020-12, subschemas inlined, root title and description dropped): flat objects of strings, integers, booleans, string arrays (the queue tools: one level of closed inline objects); optionals nullable `[T, "null"]`; `additionalProperties: false` (unknown arguments refused); no root `anyOf`/`oneOf`/`allOf`, no `$ref`; `kinds` free strings, never an `enum`; `with` the enum `["links"]`. Output from mirror types of the CLI's `--json` documents (`mirror.rs`; the CLI must not depend on rmcp or schemars), schema only, never built: every key `required`, absent = `null`; closed CLI enums mirrored (`origin`, `state`, `direction`, `form`, `mark`, layer keys), kinds and link types free; bundle `task` `{"type": "null"}`. A test-local walker checks every `structuredContent` against its schema with exact key sets: a CLI key added without its mirror turns it red.

**Texts.** `INSTRUCTIONS` (ASCII, ≤ 2 048 bytes: Claude Code truncates at 2 048 characters and defers tools behind tool search): the thirteen tools (the queue's and the tasks': only the queue written), REF forms, the flag map (`--kind` `kinds`, ROOT `root`, `--links` `with ["links"]`), the cut, the resources, determinism, "reads refresh SpecEngine's index in its data directory; nothing under the project root is written"; 1 878 B (2 035 with `probes`). Each description ≤ 2 048, ASCII, holds "Deterministic: one state, one result; no LLM inside." Compile-time asserts tie each number a text states to its CLI constant (`OUTPUT_CAP_CHARS` 40 000, `SHOW_TAIL_NAMES` 20, `MIN_TERM_CHARS` 3, `SEARCH_LIMIT_MIN`/`MAX`/`DEFAULT` 1/200/20, `DEFAULT_BUNDLE_BUDGET` 2 000): a CLI change breaks the build, not the text.

## Resources

Text only, for `@`-mentions; one inserts content without a tool call, so no hook fires (read-only: irrelevant to the gate).

- `resources/list`: `spec://<slug>/tree` (`name` `tree`, `text/plain`, ≙ `get_tree {}`) first on page 1, then every live document (CLI `documents`: neither `class: generated` nor Tier 3; an unparsed file is live, listed by path) by path in byte order: `{"uri": "spec://lantern-keep/node/docs%2Fspec%2Fgame.md", "name": "DOM-GAME", "title": "Lantern Keep", "mimeType": "text/markdown"}` (`name` the ID, else the path; no title → omitted; all but RFC 3986 unreserved bytes percent-encoded). 200 entries per page, the tree among page 1's; `nextCursor` = the page's last path, the next page starts after it.
- Template `{"uriTemplate": "spec://{project}/node/{id}", "name": "node", "mimeType": "text/markdown"}` ≙ `get_node {id}`: `{id}` = everything after `node/`, percent-decoded once as UTF-8 (either hex case, a raw `#` kept), any REF.
- List, template list and read carry `ttlMs: 0`, `cacheScope: "public"` for 2026-07-28 peers; legacy capability `resources: {}`.
- Read errors: another slug, neither form, or a REF naming nothing (CLI exit 1: its `spec:` line) → not found, `data: {"uri"}`: -32002 legacy, -32602 from 2026-07-28 (rmcp rewrites the code, SEP-2164; `data.uri` still tells it apart); a bad `%` sequence or a non-UTF-8 `{id}` → -32602 without `data`; CLI exit 2 → -32603 with its line(s).

## No project, discovery, state

The server starts in both eras with or without a project. **No project** = CLI `locate` fails: an unusable current directory or `--root`, no `specengine.toml` by the walk or in `--root`. Then each tool returns the CLI's exit-2 line (naming `spec init`, or the `--root`), `resources/list` `[]`, a read -32603; an unusable `HOME` likewise (`[]`). Any failure once a project is found → `resources/list` -32603 with the CLI's line: the config unreadable, not UTF-8 or invalid, no slug, the database busy, corrupt or not writable. The CLI's order holds, config → slug → `HOME`: a broken config or a missing slug with `HOME` unset is -32603. Following the CLI's split, `--root nope` → `[]`, `--config nope.toml` → -32603 (`locate` does not read the `--config` path).

Each request finds the project anew from the process's current directory and the globals (`Env::from_process`), runs on the blocking pool (`spawn_blocking`) and keeps nothing (outcome, project, config, DB handle): an edit, a deleted DB, a changed `bundle_node` apply on the next call. A cancelled call runs to its end (its only write: the index), its answer dropped; other requests are answered meanwhile (`ping` in the legacy era only: 2026-07-28 has none). Writes: where the CLI's reads write (the queue tools: the queue), the data directory (`docs/canon/architecture.md#storage`); nothing under the root; no tool exposes `init`, `index`, `check`, `export index` (08 AC-3, the single door). A quiet panic hook (`main.rs`) prints nothing: stdout carries JSON-RPC only.

## Size

`MAX_RESULT_CHARS` = 500 000 per tool, text and serialized `structuredContent` together. The bound: text ≤ 40 000 characters + one tail (≤ `SHOW_TAIL_NAMES` names per list) + note lines; JSON holds the printed items only, a character escaping into ≤ 6 (`\u00XX`). Measured maxima (text + JSON, debug, control characters): `get_node` 267 668, `get_tree` 246 644, `search` 231 083, `get_context_bundle` 246 360. `get_task`: JSON and notes within `PACKAGE_BUDGET` 460 000, the brief 40 000 (`task-package.md` "Caps"): within 500 000 whenever its other fields fit (every cap at once: 496 338). Unbounded residue (corpus defects): many holders of one ID in plain `show` JSON without `--links` (every holder's node kept: ~1 700 holders pass 500 000); `warning:` lines listing corpus defects; ID and path length; a package's agent free text of control characters escaped six-fold (~+290 000) or `"`, `\` doubled in changed files (+131 000), ~60 owner notes of 4 096 B or as many open proposals on its nodes, long corpus titles and paths, corrupt-row notes.

## Latency

AC-21, debug, scratch `HOME`: spec-a, spec-b cold 8–28 ms, warm 2–6 ms; a copy of this repository's docs (83 files) cold 178–236 ms, warm 19–72 ms; 3 000 synthetic candidates: `get_context_bundle` 886 ms, at budget 10 000 1 821 ms, `get_tree` 593 ms. The warm call over 1 s opens the CLI README's "lighter resolver input".

## Owner's check

AC-16, by hand; re-run on a Claude Code upgrade, the version written down. **Recorded 2026-10-03**: Claude Code 2.1.288, `claude --mcp-config fixtures/mcp/mcp.json --strict-mcp-config` from the repository root, `MAX_MCP_OUTPUT_TOKENS` unset.

- `${…}` expands: the DB landed in the scratch `HOME` (`$TMPDIR/specengine-mcp-home`); the owner's data directory was not created. `/mcp`: 7 tools, the `probes` set.
- cwd = the session's: `get_tree {}` used this repository's config.
- The four tools work: `search` "bundle budget" 12 hits; `get_node ADR-0031` ± `with: ["links"]`; `get_context_bundle ADR-0031` budget 2 000: 385 tokens.
- `structuredContent` reaches the model: it gave `end_line` 18, `utf8` `true` of `get_node ADR-0031` (JSON only; `wc -l` 18).
- Content only (`probe_output`, no lever): 48 000 characters inline; 60 000, 104 000, 200 000 stored in a file, `result (<N> characters across <M> lines) exceeds maximum allowed tokens`: the content cap lies in 48 000–60 000 (Phase 0: 48 000 in, 104 000 out).
- Above it in total, `get_node` of 05 (text ≈ 40 000 characters, `truncated: false`, the same text in `structuredContent`, whose `truncated`, `sections` the model reported) arrived inline: `structuredContent` uncounted or the cap raised by the declared 500 000, not distinguished. Read tools pass at this size: Q3's fallback not triggered.
- `@specengine:` lists the template and documents by path; the `tree` resource was not seen.
- Not verified: which explanation holds (a probe with the lever; one with small text, large `structuredContent`); UTF-16 counting; the adversarial maxima (to ~268 000 combined, Size); `nextCursor` (< 200 resources here); inserting a node by `@`-mention.

## Tests

`crates/specengine-mcp/tests` (harness: its README), over scratch copies of `fixtures/spec-a`, `-b`: `mcp_default` (lists, annotations, texts), `mcp_read` (parity, errors, schemas), `mcp_resources`, `mcp_session` (discovery, freshness, cancellation, `HOME`), `mcp_door` (the single door in a git repository), `mcp_intake`, `mcp_tasks` (task tools), `mcp_determinism`, `mcp_genre` (no kind literal, no P2-3 word in the default build), `mcp_index` (tables unchanged), `mcp_quiet`, `mcp_size`; `mcp_stdio` (`probes`). CLI `show_tail.rs`, `bounds.rs`, `locate.rs`; eval `build_graph.rs` pins MCP's `[dependencies]`.

## Open

- AC-16's "not verified" items (Owner's check); 500 000 stays the working answer (A12).
- Q3's fallback (if `structuredContent` counts and the lever fails: `nodes[].text`, `body` → `null`), untriggered so far, bounds bulk text only; short-line `get_tree`/`search` JSON stays several times its text: the owner decides.
- `get_node … with: ["links"]` lists a document's own heading (`# ADR-0031`, line 12) as a self-mention, outgoing and incoming: a candidate small fix (CLI/core links), not done.
- A panic in the async wrapper, outside `spawn_blocking`, gets no response (low).
- The Size residue. `resources/list` over a busy, corrupt or read-only database is untested.
