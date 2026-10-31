---
class: spec
status: shipped
shipped: 2026-10-31
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin]
ref: RM-0045; Phase 7 step 4 (08 s2)
adrs: [ADR-0056]
---

# card-store: cards, flows, events, CLI, MCP (RM-0045)

## Why

One board must carry every trade's work, and `T-` cannot (ADR-0056): ten closed states, a worktree at claim, a terminal at approve, a pinned package, and a malformed row closes developers' gate. This slice is the lighter record: store, flows, CLI, MCP. Step 6's fields stand here without behaviour.

## Summary

What is true now is `docs/canon/cards.md`; these headings stay because code cites them.

### Data

`[[card.flow]]` gives a discipline its columns, moves, `by` and `accept`; no table means `PLAIN_FLOW`, the same reader. `cards.md` "Config".

### Who acts

`--as N` is self-declared, recorded, compared with `by`, never verified; absent, `local`. A tool acts as `agent`. A move to `ready` is a convention until the hub.

### MCP

`list_cards`, `get_card`, `create_card`, `edit_card`, `move_card`, none accepting; no tool for `assign`, `cancel` or `accept`. `INSTRUCTIONS` gained the cards line (112 B) and lost 113 B: 2 047 B. Plugin 0.23.0.

### Store

Queue step 8 -> 9 adds `cards`, 24 columns; `events`, `proposals` unchanged. One `Immediate` transaction and one event per change.

### Documents and events

`cards.md` "Documents and events".

### Backup

Format 3: eight header keys at schema 9; formats 1 and 2 still restore.

### Rules and edge cases

Refusals exit 1 naming what is open; a closed card takes no write; the gate reads `tasks` alone. `cards.md` "Rules".

## Acceptance criteria

- [x] AC-01 — Seven writes on one `fixtures/card-press` card: `git status --porcelain --ignored` empty, `HEAD` unchanged, 7 `card.*` events. M: a card move writing a file turns `mcp_door` red.
- [x] AC-02 — A move outside `moves` exits 1 naming the allowed moves, row and events unchanged; `move --revision 1` after a move exits 1, likewise. M: `card_move` dropping the check.
- [x] AC-03 — `draft` to `edit` without `--as desk` exits 1 naming `desk`, `move_card` too; `--as desk` records it. M: `by` ignored.
- [x] AC-04 — `accept` piped exits 2; on a pty the assignee, a non-reviewer, `agent` exit 1, the reviewer's `y` moves it; an accepted card refuses every write; MCP sources hold no `card_accept`. M: `move_card` reaching `accept.to`.
- [x] AC-05 — No `[card]`: one flow line of five zeros, a new card in `plain/backlog`; `PLAIN_FLOW` pasted: `list --json` byte-identical. A bad `[card]`: `spec tree`, `spec check` unchanged. M: a branch on a column word.
- [x] AC-06 — Two `HOME`s replaying `new`, `move --before`: `list --json`, `export state` byte-identical, a neighbour's `revision` unchanged. M: ordering by `updated_at`.
- [x] AC-07 — A schema-8 queue opens at 9, rows unchanged; cards round-trip byte-identical; a format-2 dump restores; "Backup"'s refusals exit 2. M: `stored_rows` skipping `cards`.
- [x] AC-08 — Bad JSON in `cards.evidence`, an `id` `X`: `spec gate --json` as before, `task list` unchanged, `card show` exit 2, `card list` a note. M: `GateQueue::tasks` reading `cards`.
- [x] AC-09 — U+0421 exits 2 naming the Latin form; `C` in `[ids]` exits 2 for `spec card` alone; `list` works with no `.git`; `--due 2001-01-01` raw, no derived key; evidence `file=x`, a `data:` URL exit 1.
- [x] AC-10 — Each tool equals its twin; `INSTRUCTIONS` 2 047 B; `copy`, `plates`, `spec-a`, `-b`, `-c` run one code; no fixture word, no `blocked`, in card sources.
- [x] AC-11 — `cancel` needs `--reason`; one `card.cancelled`, the column closes up, only `--with-cancelled` lists it; later writes exit 1.

## Verification

75 new tests, `cards.rs` 52 of them; `fixtures/card-press/` 5 documents. Spec values held: 112 and 113 B, `INSTRUCTIONS` 2 047 B.

Final run, all green: fmt, clippy, probes 0; `cargo nextest run --workspace` 4200 run, 4200 passed, 71 skipped; UI lint 0, vitest 2780 passed, build 0; parity 3 of 3; `doc_pointers`, `ui_policy`, `anonymity`, `build_graph` 86 of 86.

Mutations M1 to M9 killed; M1 to M8 one per AC-01 to AC-08, red in `mcp_door`, `cards`, `queue_cards`, `mcp_cards`.

| Mutation | Criterion | Result |
|---|---|---|
| M1 A card move writing a file | AC-01 | killed |
| M2 `card_move` dropping the check | AC-02 | killed |
| M3 `by` ignored | AC-03 | killed |
| M4 `move_card` reaching `accept.to` | AC-04 | killed |
| M5 A branch on a column word | AC-05 | killed |
| M6 Ordering by `updated_at` | AC-06 | killed |
| M7 `stored_rows` skipping `cards` | AC-07 | killed |
| M8 `GateQueue::tasks` reading `cards` | AC-08 | killed |
| M9 | - | killed |

## Out of scope

The board and its routes, a page's accept among them: RM-0046. Suspect marks, `snapshot`, clearing, re-estimates, verdicts, and a rollup that ignores cancelled cards: RM-0047.

## Implementation

| Module | What it does |
|---|---|
| Model `card.rs` | `Card` and its JSON columns |
| Core `card_toml.rs` | Reads `[card]` from `DeTable` beside `flow_toml.rs`, first error by line, moves expanded, `PLAIN_FLOW` through the same reader |
| Core `card.rs` | IDs, `card_prefix_clash`, the pure `card_move`, `allowed_moves`, `closed_refusal`, `accept_refusal`, `stale_refusal`, `parse_evidence`; no code reads a column's name |
| Store `queue/cards.rs` | Step 8 to 9: `create_card`, `get_card`, `list_cards`, `card_history`, `change_card(id, seen, change, actor, now)`, one `Immediate` transaction and one event each. Refusals are `QueueError::CardRefused`. A move leaves its column at `position` 0, closes it up, then opens the target or goes last; neighbours take `position` alone, so no `revision` or event |
| Store `queue/gate.rs` | Untouched: the gate reads `tasks` alone, and a corrupt card row is named only by card reads |
| CLI `card.rs` | `open_cards` takes no git and no `open_context`; `[card]` is read here alone. `--target` shares `resolved_list`. Core decides a move before the store decides it again under the lock. `card_accept` asks through a `Consent`, then compares `revision` |
| CLI `state.rs`, `state_file.rs` | Backup format 3: eight header keys at schema 9; formats 1 and 2 still restore. `StoredCard` keeps `position` raw |
| MCP `cards.rs` | Five tools acting as `agent`, annotated as `claim_task`, none accepting; `{id, reason}` drops `structuredContent`, as `get_task` |
| MCP `server.rs` | `INSTRUCTIONS` 2 047 B |
| Plugin | 0.23.0: `read-spec` names the five tools |

### Files in scope

Model `src/card.rs` (new), `src/lib.rs`; core `src/card.rs`, `src/card_toml.rs` (new), `src/lib.rs`, `src/project_toml.rs`; store `src/queue/cards.rs` (new), `src/queue.rs`, `src/queue/state.rs`, `src/lib.rs`; CLI `src/card.rs` (new), `src/lib.rs`, `src/main.rs`, `src/state.rs`, `src/state_file.rs`, `src/task.rs`; MCP `src/cards.rs` (new), `src/mirror.rs`, `src/server.rs`, `src/lib.rs`; `plugin.json`; `fixtures/card-press/`; CLI `tests/cards.rs`, store `tests/queue_cards.rs`, MCP `tests/mcp_cards.rs` (new), and the pins of `INSTRUCTIONS`, `TOOLS` 28, `queue_schema`, `SCHEMA_PINS`.

**Readings the draft left open.** A discipline two flows list takes the first in the file. `revision` is decimal text. `allowed` holds moves only: the board finds accept through `accept.from`. `by: []` means anyone. `omitted` is `null` when nothing is cut. History is oldest first, an unparsable payload passed over. `agent` and `local` are refused as reviewers at assign and at accept. `--target` refuses a repeat, which is how "each once" has always been enforced (`resolved_list`). `path=` evidence takes lower-case hex of any length. `<n>` in a flow line counts the listed cards, so a filter moves it.

**Divergences from the draft.** The `by` refusal reads ``is `lead` or `owner`'s, not `local`'s``; the closed one names `spec card new --corrects`. `--as` and `--revision` off their grammar exit 2.

**Iteration 2, from the review.** A column that does not read marks its row unreadable and never fails a list. `list` notes each stranded (flow, column) once with a count, keeping the first ID. `show` prints no `notes:` line. `-` is no assignee on `new`. A card cannot correct itself; help reads "The card it corrects, never itself.". `read-spec` names the five tools.

**Left to `board-ui`:** `card_list` needs a `View`, `CardCause` a `Usage`; "nothing asked" is a `CliError`, so `CardOutcome::exit` needs a route for `Usage` that stays exit 2 on the CLI.
