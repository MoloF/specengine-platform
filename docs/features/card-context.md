---
class: spec
status: shipped
shipped: 2026-11-12
scope: [crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, plugin, scripts, fixtures, ui]
ref: RM-0108, v5 core slice 1 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the owner's vision of 2026-11-11 ("documents indexed as the AI's context"); the analyst's analysis of 2026-11-12 (CC1-CC8); the orchestrator's working answers, the owner's review open
adrs: []
---

# card-context: an agent reads a task with its context in one call (RM-0108)

## Why

The owner's vision of 2026-11-11 asks for documents indexed as the AI's context (`08-roadmap.md` Phase 7, "Owner's vision, 2026-11-11"). An agent on a task made four kinds of call: `get_card`, then `get_context_bundle` on its targets, `get_node` per decision record, and `get_card` on the parent for its goal. `get_task` gives its bundle by reference on purpose (`docs/canon/task-package.md` "Package"), and a card had nothing at all.

Now the agent asks `get_card {card_id, context: true}` once and gets the task, its documents and the decisions taken on it as one bundle, and the parent's goal. The key is opt-in, so a card read without it costs what it cost before. Nothing changes in the UI.

No ADR: the slice rests on accepted decisions. ADR-0027 says a package carries "the bundle within budget", ADR-0056 keeps a card a light queue record that reads, ADR-0046 says reads never fail on the code layer, and ADR-0012 says nothing is blocked (a context that cannot be made is a note, never a refusal).

## Description and interactions

Each decision is canon now, in `docs/canon/card-context.md`: D1 opt-in and inline, D2 one key in three parts, D8 live text: "The rule"; D3 one bundle of the targets then the card's own applied records, D4 the budget (`bundle_task`, never `bundle_node`), D7 the parent's goal: "The rule"; D5 the room: "The room"; D6 failures are notes: "Notes and exits"; D9 everywhere `get_card` is, D10 plugin 0.39.0: "MCP, daemon, hub, plugin".

The owner's questions, open, their working answers built: **Q-A** inline and opt-in; **Q-B** in `v5`, answered by his word of 2026-11-12; **Q-C** `bundle_task`, no `bundle_card` key; **Q-D** a subtask's records stay off its parent; **Q-E** a gone REF nulls the bundle, with a note.

## Data

In the canon: `card-context.md` "The rule", "The room", "Text and JSON", "Notes and exits", "MCP, daemon, hub, plugin"; `cards.md` "Commands", "Documents and events"; the key set `Card`, 35 keys (`fixtures/daemon-keys.json`, 122 sets).

## Rules and edge cases

In the canon: `card-context.md` "The rule", "Notes and exits", "Known limits".

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turned red. The new Rust tests are `card_context.rs` in `crates/specengine-cli/tests` and `crates/specengine-mcp/tests`, over scratch git copies of `fixtures/spec-a` (`[decision_records]`), with records made as `card_questions.rs` makes them. The test-engineer records the baseline texts of CC2 at 3892f03 first.

- [x] CC1 — **The bundle `spec bundle` makes.** C-0001 has two targets T1, T2 and these items: a decision applied with record R1, an open question, a rejected decision. `spec card show C-0001 --context --budget 3000 --json` has `context.bundle` equal, as a JSON value, to `spec bundle T1 T2 R1 --budget 3000 --json`, with `refs` `[T1, T2, R1]`. The text ends with `context.bundle: <that hash>` and then that body, byte for byte. Without `--budget`, `budget` is 10000. With `[budgets] bundle_task = 1500` it is 1500, equal to `spec bundle … --budget 1500`. With `bundle_node = 900` alone it is 10000. A second read is byte-equal, and so is a copy at another root and `HOME` (text and JSON). Mutations, each red: the first target only; `bundle_node` read for the budget; records before targets.
- [x] CC2 — **Not asked, nothing read.** Plain `show` and `--json` are the baseline plus the last line `context: -` and `"context": null`, every other key equal. `get_card {card_id}` is its baseline plus the same. Then every `.md` of the copy goes to mode 000 and `.git` is moved aside, after the card is made. `show C-0001` still exits 0 with no stderr line. With `--context` it exits 0 with the card's lines, `context.bundle` `null` and one `context:` note. Mutation, red: the context always built.
- [x] CC3 — **The room.** (a) A card whose criteria make its lines about 25 000 characters, with three targets whose bundle at 10 000 would print more than the room. The text before the body plus `context.bundle.chars` stays ≤ 40 000, and `context.bundle.chars` ≤ 40 000 − U ("Text"). The body is shorter than `spec bundle`'s at the same budget alone. (b) A subtask with a 4 096 B goal, 32 criteria of 1 024 B and 64 targets, whose parent's goal is 4 096 B. It exits 0, the text stays within 40 000 characters (plus the truncated tail if cut), `context.bundle` is `null`, and there is one `no bundle:` note. Mutation, red: the bundle fitted against 40 000 instead of the room ((a) passes 40 000).
- [x] CC4 — **Failures are notes.** On CC1's card, each case exits 0 with the card's document:
  - (a) T2's file deleted: `context.bundle` `null`, `context.notes` `["no bundle: <spec bundle's reason naming T2>"]`, stderr `note: context: no bundle: …`, and `get_card` not `isError`;
  - (b) `--budget 1`: the note names the minimum;
  - (c) `[budgets] bundle_task = 0`: a note naming its line, and with `--budget 3000` the bundle is made;
  - (d) a manual card: `no bundle: no targets and no recorded decisions`.

  These exit 2 with nothing read: `--budget 0`, `--budget 4294967296` and `--budget 5` without `--context`; MCP `budget` without `context` is `isError` with the library's line. Mutations, each red: the bundle's exit passed through ((a), (b)); the config error exit 2 ((c)).
- [x] CC5 — **The parent's goal.**
  - A hand-off C-0002 (own goal G2) of C-0001 (goal G1): `context.parent` is `{id: "C-0001", title, goal: G1}`, with the lines `context.parent: C-0001 | <title>` and `context.parent.goal: G1`.
  - C-0001 itself: `parent: null`, `context.parent: -`, no note. A parent without a goal: `goal: null`, `-`.
  - The parent's row hand-corrupted: `parent: null`, the note `parent C-0001 not read: …`.
  - Its goal hand-edited to 5 000 bytes: cut within 4 096 bytes on a character boundary, with its note.

  Mutation, red: the card's own goal.
- [x] CC6 — **Only the card's own applied records.** C-0001's items are a decision applied with R1, a question applied with R2, an open decision, a rejected one, and a discrepancy applied with no record. Its subtask C-0002 has a decision applied with R3, which shows in C-0001's `subtask_items`. C-0001's `refs` are its targets, then R1 and R2 by item ID. C-0002's include R3. The cap is a unit test of the selection (`#[cfg(test)]` in `card.rs`): 66 applied records give 64 REFs and the note `not bundled: 2 more records, from <the 65th's ID>`. Mutations, each red: open items taken; `subtask_items` taken; no cap.
- [x] CC7 — **Parity, schemas, the hub.** The stdio `get_card {card_id, context: true, budget: 3000}` content is byte-equal to `spec card show C-0001 --marks --context --budget 3000 2>&1`, and its `structuredContent` equals the `--json`. The same holds with `history: true` and in CC4 (a). The schema walker passes with exact key sets: `context` `{parent, bundle, notes}`, `parent` `{id, title, goal}`, `bundle` the bundle document's 13. The input schema is `card_id`, `history`, `context`, `budget`, with `additionalProperties: false`. On a hub's `/mcp/<slug>` the same call answers alike, `tools/list` holds 18 tools, and `INSTRUCTIONS` is byte-equal to 3892f03's. Mutations, each red: a CLI key without its mirror (`notes` dropped from `CardContextShape`); `context` ignored on the hub's route.
- [x] CC8 — **Plugin 0.39.0.** `plugin.json` is 0.39.0, `.env.example` `SPECENGINE_VERSION=0.39.0`, and `tag-check.sh v0.39.0` prints `0.39.0`. `PINS` and `SCHEMA_PINS` end with 0.39.0 entries. Mutation, red: the schema changed with no new `SCHEMA_PINS` entry (`tool_schemas_follow_the_plugin_version`).
- [x] CC9 — **The texts.** `GET_DESCRIPTION` is as "Data" gives it: 1 987 B, ASCII, its numbers tied by asserts. `read-spec` has body 4 091 B and file 4 412, the sentence once in the cards bullet, the bullet's start and end kept, no digit, and its description unchanged. Stdio's `INSTRUCTIONS` is byte-equal to 3892f03's. Mutations, each red: `DEFAULT_TASK_BUDGET` set to 10 001 (the build fails); the sentence dropped.
- [x] CC10 — **The daemon serves null; the UI types it.** `GET .../cards/C-0001` serves `context: null` among 35 keys. `fixtures/daemon-keys.json` `Card` names `context` after `history`, in 122 sets. `provisional.ts` `Card.context: null`, citing this criterion. UI lint has 0 problems, `tsc` is clean, and `pnpm test` is green, `daemonKeys.test.ts` included, with `AHEAD_OF_FIXTURE` empty at the end. Mutations, each red: the route asking `context: true`; `context` out of the fixture (`daemon_keys.rs`).
- [x] CC11 — **[measure] Size and time.** `mcp_size` measures `get_card {context: true, history: true}` on CC3 (b)'s card and on CC3 (a)'s with 64 items, each with a 2 048 B `why` and a long summary. Text plus JSON stays ≤ 500 000 characters, and the maximum is pinned in the test and recorded in "Implementation". `--context` on a scratch copy of a real corpus: wall time warm, recorded.
- [x] CC12 — **Gates.** `cargo nextest run` for `specengine-cli`, `-mcp`, `-http` is green, then the full run once. The eval tests (`doc_pointers`, `ui_policy`, `anonymity`) and `spec check` pass. UI lint, `tsc`, `pnpm test` and `pnpm build` pass, with first-load JS unchanged within 0.1 kB.

## Out of scope

An inline bundle for `get_task` and a subtask's records on its parent are out. So are any UI that shows a context, a new tool or `[budgets]` key, a context on `list_cards` or on the daemon's route, and a context cached or stored. A REF that does not resolve stays a known limit: it is never dropped alone while the rest is bundled.

## Implementation

The Rust review ACCEPT, four nits: `bundle.rs`' module doc gained `bundle_within`'s clause; the other three are Known limits (`card-context.md`). CC12 is the owner's full run.

| Module | Does |
|---|---|
| cli `bundle.rs` | `chars_max` through `run_bundle`, the assembly and `fits`; `bundle_within`; `budget_of` `pub(crate)` |
| cli `card.rs` | `CardShowRequest.{context,budget}`; `CardContext`, `CardContextParent`; `context_{parent,refs,budget,room,bundle}`, `add_context`; the `context` lines; the internal open state renamed `OpenCards`, with `config_text` |
| cli `lib.rs`, `main.rs` | the exports; `--context`, `--budget` |
| http `app.rs` | the daemon asks none: `context: false` |
| mcp `cards.rs`, `mirror.rs` | `GetCardArgs.{context,budget}`; `GET_DESCRIPTION` 1 987 B, `holds_number` asserts; `CardContextShape`, `CardContextParentShape` |
| `plugin/`, `scripts/deploy/.env.example` | 0.39.0; `read-spec` body 4 091 B, file 4 412 |
| ui `api/provisional.ts`, `mocks/cards.ts`, `test/builders.ts` | `Card.context: null`, rendered nowhere |

**Departures, settled.** (1) An exit-2 bundle line enters its note without `spec: `, as a package's. (2) No clap `requires`: the library's `` `--budget` needs `--context` `` runs first, before `budget_of`'s range, so the CLI and MCP print it alike. (3) `escape_controls` runs over the whole text, body included: a Known limit. (4) The document's top-level `notes` are unchanged; the context's notes are in `context.notes` and on stderr. (5) REFs are passed un-de-duplicated; the bundle merges them. (6) CC2's baselines come from 82a7d39's binary: no code changed since 3892f03. (7) `read-spec` now names `get_context_bundle` twice, so `plugin_skills`' `ac06` helper was retargeted. (8) CC6's applied discrepancy without a record cannot be stored (a corrupt row): a row set in SQL and the unit test.

**Numbers.** CLI 298 tests green (`card_context.rs` 17), mcp 86 (`card_context.rs` 5), http 84, eval 21 + 35, vitest `daemonKeys` 385, clippy clean; 19 named mutations red. CC11: `CARD_CONTEXT_MAXIMUM` 424 208 characters (text 39 805, JSON 384 403, 64 items), a subtask 96 131. A scratch copy of a 375-document real corpus, debug, warm, 3 targets: `show --context` 0.48-0.51 s, plain `show` 0.01 s, `spec bundle` of the same REFs 0.47-0.50 s. Room probes: `U` 39 724 leaves 276, 39 993 leaves 7; large cards 39 964, 39 997, 39 968 characters.

**Gaps**: `card-context.md` "Tests"; **Known limits**: there.
