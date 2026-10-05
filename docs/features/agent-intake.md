---
class: spec
status: shipped
scope: [crates/specengine-mcp, crates/specengine-cli, crates/specengine-store, crates/specengine-core]
ref: agent-intake analysis 2026-10-05; 08 §2 Phase 2, slice 3
shipped: 2026-10-05
---

# Agent intake over MCP

## Why

Agents raised only `update`, through Bash; MCP was read-only, so discrepancies and questions stayed prose in chat and the owner re-answered settled ones (08 AC-5). Now each becomes a queue record with evidence and priced options, checked against what is decided, while the agent works on its working answer (ADR-0012). No new ADR: the stdio server writes the queue through the CLI library, per request, the interim `proposal-queue.md` "Store" grants CLI processes until the daemon (ADR-0019).

Working answers (the owner's, 2026-10-05, session rule): Q1 intake only, decisions in `decision-apply`; Q2 questions live in the queue as `PR-…`, no record file; Q3 the owner settles one by `spec reject PR --reason <answer>`, returned by `get_proposal` and dedup; Q4 dedup deterministic, search never a hit; Q5 `author_role` required, verbatim, no enum; Q6 one level of closed inline objects in input schemas, no `$ref`; Q7 bound to the server's root.

How it works now: `docs/canon/agent-intake.md` (tools, schemas, CLI twins, rules, dedup, documents, stored, settling, authors, size, known limits).

## Acceptance criteria

Setup: temp git repos of `fixtures/spec-a` (`mcp_genre.rs`'s garden if named), scratch `HOME`, injected clock; MCP: the default build as `mcp_*` spawn it; library approve: consent yes. Refused: nothing stored, no event, next ID unchanged. M: the mutation turning it red.

- [x] AC-01 — each default-build tool, a valid and an invalid call (a change to `RULE-STAM-REGEN`, a discrepancy with a patch): `git status --porcelain` empty, files byte-identical, no commit (`mcp_door.rs`; M: `propose_change` writes its target).
- [x] AC-02 — `open`, `approved` with its commit on its branch, `applied`, `rejected`: `get_proposal`, every write tool: statuses, `decided_*`, `applied_commit`, `events()` unchanged; `tools/list` = the four reads + these four (M: `get_proposal` via approve's completion lookup).
- [x] AC-03 — no `author_role`: `isError` naming it, refused; `"nest-developer"` alone → `{"type":"agent","role":"nest-developer","model":null,"run":null}`; `"a b"` refused; the twin without `A`: `human` (M: `author_role` optional, absent passed on).
- [x] AC-04 — `EDGE-STAM-ZERO` asked twice, whitespace and case apart: `created: false`, hit the first ID, `counts()` unchanged; after `reject --reason R`: hit `rejected`, answer `R`; `RULE-STAM-REGEN`, `Q-031`: hit `DEC-0023`, its title, nothing stored; the text on another node: created; `distinct_from` naming every hit: created, in review; a subset: not; a temp accepted decision only mentioning the node: `related`, created (M: dedup off; targets ignored; any `distinct_from`; mentions as hits).
- [x] AC-05 — 8 parallel identical `ask_question` + the twin: one row, every answer naming it (M: the queue read before `BEGIN IMMEDIATE`).
- [x] AC-06 — refused naming the field: 1 and 7 options, `recommendation` 2 of 2, blank `working_answer`, `price_of_other`, no `node_ids`, an unknown ID, alias `QST-031` (exit 1 naming `Q-031`), a U+0420 look-alike (exit 2), an evidence field over cap, an unknown argument; then a valid ask stores `PR-0001` (M: `options` optional; a cap dropped).
- [x] AC-07 — a patch citing an undeclared ID: both stored, `linked` both ways, the update's `diagnostics` and brief with it; a stale `base`: refused naming `proposed_patch` (M: refused on findings; a `linked` missing).
- [x] AC-08 — ESC, U+0085, U+202E, U+2066 in a summary, working answer, `observed`, label: `inbox`, `review` text escaped, JSON raw; `inbox`: `… | question | open | EDGE-STAM-ZERO | <branch> | <created_at> | normal: <text>` (M: one field raw).
- [x] AC-09 — library approve on a question, a discrepancy: exit 1 naming `spec reject`, no prompt, `dump()` unchanged; `approve_from` → `Invalid`; reject: `rejected`, reason in `decision_note` (M: approve accepted).
- [x] AC-10 — a v1 DB opened: `user_version` 2, rows kept, the eleven NULL; every kind exported, imported fresh: `dump()` equal, re-export byte-identical; a `queue_schema` 1 dump imports, re-exports as 2; 3: `upgrade SpecEngine` (M: schema 1 refused; a column left out).
- [x] AC-11 — `mcp_genre.rs`, core `check_genre.rs`, eval `build_graph.rs` green; garden: ask, ask again, `get_proposal`: no P2-3 word (M: a `"question"` literal in `specengine-mcp/src`).
- [x] AC-12 — "Tools" holds (no `$ref`, inline objects closed); a 1 MiB `propose_change`, a discrepancy at every cap of control characters: each result ≤ `MAX_RESULT_CHARS`, `content` ≤ 48 000 (M: `readOnlyHint: true`; the brief keeping `new_text`).
- [x] AC-13 — `content` byte-equal to the twin's `2>&1`, `structuredContent` its `--json`; made with `HOME` X: in `spec inbox` with X, not Y (M: the server's own data directory).
- [x] AC-14 — gate clean; worst W ≤ min(109 484, 108 729); the new canon ≤ 12 288 B, READMEs ≤ 10 240 B; `CLAUDE.md` not grown; anonymity green (M: a byte added to `CLAUDE.md`).

## Implementation

Canon: `docs/canon/agent-intake.md` (new); pointers in `proposal-queue.md` ("Commands"; "Store": schema 2, `create_intake`, the interim line extended to the MCP stdio server), `queue-backup.md` ("Format", "Import", "Store"), `mcp-read.md` ("Binary", "Schemas", "Texts", "Tests"), the core, store, CLI, MCP READMEs; 05 §3.3, §7; 06 §3.2–3.4; 07 §1.2, §2; 08 Phase 2, AC-5. Two iterations, both reviews accepted; check, fmt, clippy (`probes` too) clean.

| Module | What it does |
|---|---|
| core `intake.rs` (new), `proposal.rs` | kinds, enums, input types, caps, field checks in input order, `author_problem`, `normalized_summary`; `TEXT_MAX_BYTES` moved here |
| store `queue.rs`, `queue/state.rs` | schema step 2, `ProposalKind::applies`, `Intake`, `NewIntake`, `QueueMatch`, `IntakeResult`, `create_intake`, kind-aware decoding, `Invalid` guards; 35 columns, `proposal_columns(schema)` |
| CLI `intake.rs` (new) | requests, `read_discrepancy_input`, steps 2–7, corpus hits from the spec graph, the intake document and text |
| CLI `propose.rs`, `proposals.rs`, `review.rs`, `inbox.rs`, `apply.rs`, `state_file.rs`, `main.rs`, `lib.rs` | shared steps 1–4, `propose_brief`, `review_brief`, author and rationale checks; the eleven review keys, `briefed`, `cut_brief`; inbox `severity`, `summary`; approve refusal, reject without history; dumps of schema 1 and 2; the commands, `Outcome::Intake`, `utc_now`, `process_git` |
| MCP `intake.rs` (new), `mirror.rs`, `server.rs`, `read.rs` | the four queue tools, their mirrors, the `intake_tools` router, `INSTRUCTIONS` |

Tests: store `queue_intake.rs`; CLI `intake.rs`, `intake_state.rs`; MCP `mcp_intake.rs`, `mcp_door.rs`, `mcp_genre.rs`.

Accepted deviations, now canon. Iteration 1: kind words as core constants; a bad `severity`, `gap_type` is a form error (exit 2); a given blank optional refused; step 3–4 refusals prefixed `node_ids[i]: `, `proposed_patch: `, a node twice refused; the patch target resolved before the among-check; rejecting a new kind still reads the committer; brief tail `[truncated: …]`; inbox summary cut at 80; `INSTRUCTIONS` shortened; queue tables at an unknown `user_version` → `Invalid`. Iteration 2: `DISTINCT_MAX` 16 → 64, matches past 10 named up to 64 per list (`unnameable`); `--input` errors escaped; `create` refuses a never-applying kind; a bad author field: `propose update` exit 2, intake exit 1, kept as is.

Recorded (canon "Known limits"): unnameable hits (a path over 256 bytes or with a control character) block storing; `unnameable` counts documents, not names; names joined by `, `; path-length residue. Asking about a whole document also hits decisions linked to its sections (`spec show --links` lists them), as the dedup rule reads. Code comments cite the canon `agent-intake.md` (`queue-backup.md` "Format" for the schema-1 restore), not this spec.
