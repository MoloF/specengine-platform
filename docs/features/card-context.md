---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, plugin, scripts, fixtures, ui]
ref: RM-0108, v5 core slice 1 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the owner's vision of 2026-11-11 ("documents indexed as the AI's context"); the analyst's analysis of 2026-11-12 (CC1-CC8); the orchestrator's working answers, the owner's review open
adrs: []
---

# card-context: an agent reads a task with its context in one call (RM-0108)

## Why

The owner's vision of 2026-11-11 asks for documents indexed as the AI's context (`08-roadmap.md` Phase 7, "Owner's vision, 2026-11-11"). An agent on a task today makes four kinds of call: `get_card`, then `get_context_bundle` on its targets, `get_node` per decision record, and `get_card` on the parent for its goal. `get_task` gives its bundle by reference on purpose (`docs/canon/task-package.md` "Package"), and a card has nothing at all.

After this slice the agent asks `get_card {card_id, context: true}` once and gets the task, its documents and the decisions taken on it as one bundle, and the parent's goal. The key is opt-in, so a card read without it costs what it costs today. Nothing changes in the UI.

No ADR: the slice rests on accepted decisions. ADR-0027 says a package carries "the bundle within budget", ADR-0056 keeps a card a light queue record that reads, ADR-0046 says reads never fail on the code layer, and ADR-0012 says nothing is blocked (a context that cannot be made is a note, never a refusal).

## Description and interactions

### Decisions

- **D1. Opt-in, inline.** `spec card show C [--history] [--marks] [--context [--budget N]]`; MCP `get_card {card_id, history?, context?, budget?}`, its content byte-equal to the CLI's `2>&1` (`docs/canon/mcp-read.md` "Parity"). No new tool: `INSTRUCTIONS` has no bytes left, and the key needs none.
- **D2. One key, three parts.** `context` is `{parent, bundle, notes}`. It is `null` when not asked, and every part is present when asked.
- **D3. One bundle.** Its REFs are the card's `targets`, in stored order, then the `record_id` of each of the card's own items that is `applied` and holds a record, in `items` order. That is one fitting and one `bundle_hash`. A subtask's records stay out of its parent's bundle: their IDs are in `subtask_items`.
- **D4. The budget.** `--budget N`, else `[budgets] bundle_task`, else `DEFAULT_TASK_BUDGET` 10 000, as a task's bundle. No new `[budgets]` key. `bundle_node` is never read for a card.
- **D5. The room.** The card's text plus the bundle stay within `OUTPUT_CAP_CHARS` (40 000): the bundle is fitted into the room the card's lines leave. Claude Code moves content above 48 000-60 000 characters into a file (`mcp-read.md` "Owner's check"), so the 40 000 ceiling binds, not the 500 000 lever.
- **D6. Failures are notes.** A bundle that cannot be made (a REF that does not resolve, a budget below the frame's minimum, a frame over the room, a bad `bundle_task`, an index error) leaves `bundle` `null` with a note. The card read keeps its exit and its document (ADR-0046, ADR-0012).
- **D7. The parent's goal.** `parent` is `{id, title, goal}` of the card's direct parent, read from the same queue. With no parent it is `null` and carries no note.
- **D8. Live text.** The bundle reads the corpus as it is now, never the card's `snapshot`. The card's `mark` already says when its brief changed.
- **D9. Everywhere `get_card` is.** The hub's `get_card` is stdio's route as it is (`SHARED`), so it takes the key with no hub code. The hub's `INSTRUCTIONS` and its 18 tools are unchanged. The daemon's `GET .../cards/:id` never asks, so it serves `context: null`. The UI gains one typed key and no behaviour.
- **D10. Plugin 0.39.0.** A key a tool takes (`get_card`'s `context`, `budget`) and a key it returns (`context`) is a MINOR. `read-spec` names the key, with `get_context_bundle` as the fallback for an older server.

Every name in "Data" is fixed, so the lanes never wait on each other. The UI lane needs only the daemon's `context: null`: one typed key that nothing renders, in before the fixture changes ("Work order").

## Data

### Command

```text
spec card show C [--history] [--marks] [--context [--budget N]]
```

- `--context`: build `context`. Without it, `context` is `null` and the text's last line is `context: -`.
- `--budget N`: estimated tokens, 1..=4294967295, as `spec bundle`'s. It requires `--context`. Without `--context`, `0` or out of range: exit 2, a usage error, before anything is read. The library checks the same (`` `--budget` needs `--context` ``; the range line is `spec bundle`'s `budget_of` text), so MCP gets the same line.
- Exits are unchanged: 0 answered (a context with notes included), 1 no such card, 2 as today plus the two usage errors above.

Library: `CardShowRequest` gains `pub context: bool` and `pub budget: Option<i64>`. Every construction sets them: CLI `main.rs` from the flags, MCP `cards.rs` from the arguments, http `app.rs` `false` and `None`. `CardShowOutcome` gains `pub context: Option<CardContext>`:

```rust
pub struct CardContext {
    pub parent: Option<CardContextParent>, // the direct parent, read now
    pub bundle: Option<BundleOutcome>,     // made: `bundle` is Some, `reason` None
    pub notes: Vec<String>,                // each printed as `note: context: <note>`
}
pub struct CardContextParent { pub id: String, pub title: String, pub goal: Option<String> }
```

Both are exported from `lib.rs`. `BundleRequest` and `bundle()` are unchanged (19 construction sites). The ceiling is a parameter of the crate-internal `run_bundle`: `bundle_with` passes `OUTPUT_CAP_CHARS`, and a new `pub(crate) fn bundle_within(env, globals, request, chars_max)` serves the card. "Fits" becomes `chars ≤ chars_max`. The frame-over-ceiling error names the ceiling it was given.

### Text

The card's lines are today's `key: value` lines (`docs/canon/cards.md` "Commands", `show`), with one line or one block added after `history`:

```text
history: -
context: -
```

When asked:

```text
history: -
context.parent: C-0001 | Write the tide table foreword
context.parent.goal: Readers learn the tides before the tables.
context.bundle: b3:<64 hex digits>
# Bundle: MEC-TIDE-TABLES, DEC-0031

## Targets
<the bundle's body, as `spec bundle` prints it, without its two closing lines>
```

- `context.parent:` is `<id> | <title>`, or `-`. `context.parent.goal:` is the goal through `one_line`, or `-` (no parent, or no goal).
- `context.bundle:` is the bundle's `bundle_hash`, or `-` when `bundle` is `null`. The body follows only when one was made, and it is always last. Tokens, characters and bytes are in the JSON (`context.bundle.tokens` …).
- The context's notes are messages, `note: context: <note>`, printed after every message the read prints today. They are followed by the bundle's own messages (`note:`, `warning:` lines, verbatim, as `spec bundle 2>&1` shows them). As every note, they sit outside the cut.

**The room.** Let `U` be the sum, over every line the text prints before the body, of (its characters + 1). The `context.bundle:` line is counted at its made length, 84 (`context.bundle: ` 16, the hash 67, `\n` 1). The bundle is fitted with `chars_max = 40 000 − U`. Then:

- if the card's lines are cut (the `[truncated: …]` tail prints), or `U ≥ 40 000`: no bundle is asked, and the note is `no bundle: the card's lines leave no room under the 40000-character ceiling`;
- otherwise text before escaping ≤ 40 000 characters, since `chars(body) ≤ chars_max`.

### JSON (`--json`, `get_card`'s `structuredContent`)

The card document gains `context` between `history` and `notes`: 35 keys.

```json
"context": {
  "parent": {"id": "C-0001", "title": "Write the tide table foreword", "goal": "Readers learn the tides before the tables."},
  "bundle": {"refs": ["MEC-TIDE-TABLES", "DEC-0031"], "reason": null, "notes": [], "task": null,
             "budget": 10000, "tokens": 812, "chars": 3204, "bytes": 3212, "bundle_hash": "b3:…",
             "body": "# Bundle: MEC-TIDE-TABLES, DEC-0031\n\n## Targets\n…", "layers": {…}, "tail": [], "more": 0},
  "notes": []
}
```

- `parent` is `null` or `{id, title, goal}`, with `goal` `null` when the parent has none.
- `bundle` is `null` or the `spec bundle --json` document, exactly (`docs/canon/spec-cli-bundle.md` "Output"; 13 keys). It is serialized by the bundle's own `view`, never a copy, so CC1 compares it as a value. It is never the exit-1 document: a bundle not made is `null` and a note.
- `notes` are strings without the `context: ` prefix.

### Targets, budget, notes

- **REFs:** `card.targets` as stored (canonical, at most 64, `NODES_MAX`), then the records. A record is an item of `items` with `status` `applied` and `record_id` set, of any kind (a question's answer, a discrepancy's, a decision). The `record_id` is the REF. At most 64 records (`NODES_MAX`), the first by `items` order; past them the note is `not bundled: <k> more records, from <first left-out ID>`. A REF that is also a target counts once, as `spec bundle` merges it.
- **Budget:** `--budget N`, else `bundle_task_from_toml` over the reading root's config text (read alone, as `package.rs` `bundle_of`), else `DEFAULT_TASK_BUDGET`. It is passed as `BundleRequest.budget = Some(n)`, so `bundle_node` stays unread, and a too-small budget's message names `--budget N`, as a task's does.
- **Parent:** `card.parent`, read by `queue.get_card`, only when asked. A `goal` over `GOAL_MAX` (4 096 bytes: a hand edit) is cut at the last character boundary within it, with the note `parent <ID>'s goal cut at 4096 bytes`. An unreadable parent gives `parent: null` and the note `parent <ID> not read: <why>` (`no card <ID>`, or the queue's corrupt-row line).
- **Notes, exact, in this order:**

| When | Note (printed `note: context: …`) | `bundle` |
|---|---|---|
| the parent cannot be read | `parent C-0001 not read: no card C-0001` | as below |
| the parent's goal over 4 096 B | `parent C-0001's goal cut at 4096 bytes` | as below |
| more than 64 records | `not bundled: 3 more records, from DEC-0101` | as below |
| no targets and no records | `no bundle: no targets and no recorded decisions` | `null` |
| no room | `no bundle: the card's lines leave no room under the 40000-character ceiling` | `null` |
| a bad `bundle_task` | `no bundle: <the config error, at its line>` | `null` |
| `spec bundle` exit 1 | `no bundle: <its reason>` | `null` |
| `spec bundle` exit 2 (the minimum, the frame over the room, `HOME`, store, index) | `no bundle: <its error line, one_line>` | `null` |

A card read without `context` reads no config for a budget, no parent and no bundle. The CLI's plain `show` therefore stays a queue read: no git (unless a task is linked, as today) and no corpus. `get_card`, which always asks `--marks`, keeps its marks' index read, as today.

### MCP: `get_card`

```rust
pub(crate) struct GetCardArgs {      // deny_unknown_fields, as today
    pub card_id: String,
    pub history: Option<bool>,
    /// Its context too: the parent's goal and one bundle of its targets and records.
    pub context: Option<bool>,
    /// Estimated tokens for context's bundle, 1 to 4294967295; else the project's
    /// [budgets] bundle_task, else 10000. Needs context: true.
    pub budget: Option<i64>,
}
```

`context: true` maps to `context: true`; absent, `null` or `false` maps to `false`. `budget` without `context: true` is the library's exit 2: `isError`, its line, no `structuredContent`. The output schema mirrors the document (`mirror.rs`):

- `CardShowDocument` gains `context: Option<CardContextShape>`, between `history` and `notes`;
- `CardContextShape {parent: Option<CardContextParentShape>, bundle: Option<BundleDocument>, notes: Vec<String>}`;
- `CardContextParentShape {id, title, goal: Option<String>}`;
- `BundleDocument` is the existing mirror, reused.

`GET_DESCRIPTION`, 1 987 B of 2 048, ASCII. The head becomes "One card: the same as `spec card show C --marks [--history] [--context [--budget N]]`." This sentence goes after "… in one read.", before "Reads only:":

```text
context: true: {parent {id, title, goal} or null, bundle, notes}: bundle is get_context_bundle's document over its targets, then the records of its applied items, at budget (else the project's bundle_task, else 10000), fitted into the room its lines leave of the 40000 characters, its body printed after them; a bundle that cannot be made is null with a note, never an error. budget needs context. Without context: null.
```

New compile-time asserts tie "else 10000)" to `DEFAULT_TASK_BUDGET` and "leave of the 40000 characters" to `OUTPUT_CAP_CHARS`, as `holds_number` does today.

### The daemon and the hub

- Daemon `GET .../cards/:id` (`app.rs` `card`): `context: false, budget: None`, so it serves `"context": null`. Its key set `Card` is 35 keys, `context` after `history`; `fixtures/daemon-keys.json` still names 122 sets.
- Hub `/mcp/<slug>`: `get_card` takes and returns the key. The bundle reads the hub's checkout inside the project's turn (`docs/canon/hub-mcp.md`). `get_card` is not in `HUB_READ_TOOLS`, so it walks, as today. `HUB_TOOLS` (18) and the hub's `INSTRUCTIONS` are unchanged.

### The plugin

- `plugin.json` and `scripts/deploy/.env.example` (`SPECENGINE_VERSION`) go to 0.39.0; `tag-check.sh` reads them unchanged.
- `PINS` and `SCHEMA_PINS` (`plugin_files.rs`) each append a 0.39.0 entry.
- `read-spec`'s `SKILL.md`, three edits. Body 4 086 → 4 091 B (≤ 4 096), file 4 407 → 4 412, no digit, description unchanged:
  1. drop "Use this before you read or change behaviour the spec describes, and whenever you need to know what the spec says. " from the paragraph after the precedence sentence;
  2. "`get_tree`, from a `root` and to a `depth` when the whole tree is too large;" → "`get_tree`, from a `root`, to a `depth`;";
  3. in the cards bullet, after "and no tool accepts a card.", insert one space and: "`get_card` with `context: true` adds its targets' bundle, its parent's goal and its decisions; an older server refuses `context`: use `get_context_bundle`."

  The bullet still starts with "- Work of any trade stands on cards" and still ends with the vocabulary sentence.

### The UI line

- `ui/src/api/provisional.ts`: `Card` gains `context: null;` after `history`. Its doc comment adds: "`context` is always null here: the daemon never asks a card's context (`docs/features/card-context.md` "Acceptance criteria", CC10)."
- `ui/src/mocks/cards.ts`: `cardShowOf` serves `context: null`, and `StoredCard`'s `Omit` list gains `"context"`.
- `ui/src/test/builders.ts`: `aCard` gains `context: null`.
- `ui/src/api/daemonKeys.test.ts`: the `Card` record and its key string gain `context` after `history`, and the header comment names this slice. While the fixture lacks the key, `AHEAD_OF_FIXTURE` holds `["Card", "context"]`.

## Rules and edge cases

- WHEN `context` is not asked, the read SHALL be today's, plus `context: -` and `"context": null`. It SHALL read no parent, no config for a budget, and no bundle.
- WHEN asked on a card that does not exist, the read SHALL exit 1 as today, with no context.
- WHEN asked on a cancelled, accepted or stranded card, the context SHALL be built as on any card.
- WHEN the bundle cannot be made, the read SHALL exit 0 with the card's document, `bundle: null` and one note saying why. A REF that does not resolve (a gone target or record) nulls the whole bundle: one fitting, one hash. The note names the REF, and the agent bundles the rest with `get_context_bundle` (a known limit).
- WHEN the card's lines are cut, the read SHALL ask no bundle.
- The text before escaping SHALL stay within 40 000 characters, plus the truncated tail when the lines are cut, as today. Escaping control characters can still pass it, the residue `mcp-read.md` "Size" names.
- Determinism: one queue and corpus state gives one text and one JSON. Copies at different roots and `HOME`s are byte-identical. No result holds a date the read computes.
- Writes: the data directory only (the bundle's index update and code refresh). Nothing under the project root, no queue row, no event.
- ADR-0012: the context decides nothing. No move, accept, gate or rollup reads it.

### Rollout

- An older `specengine-mcp`, on a terminal or on a hub at 0.38, refuses `context` (`failed to deserialize parameters: unknown field`). `read-spec` names the fallback.
- No queue schema step, backup format, config key or daemon route change. The hub picks the key up with its next upgrade, which stays the owner's.

### Test churn

- Every `spec card show` text the CLI and MCP tests pin gains the last line `context: -`, and every card document key list gains `context` (35): CLI `cards.rs`, `card_{trace,questions,mandate,tasks,handoff,split}.rs`, MCP `mcp_cards.rs`, `card_mandate.rs`, `card_split.rs`, where they pin `show`.
- `plugin_skills.rs`: `read-spec` sizes 4 091 and 4 412, and the cards bullet. `plugin_files.rs` and `plugin_roles.rs`: the version pins. http `spec_ids_hub.rs`: `v0.39.0`. `mcp_hub.rs`: the hub's schemas, if it pins them.

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turned red. The new Rust tests are `card_context.rs` in `crates/specengine-cli/tests` and `crates/specengine-mcp/tests`, over scratch git copies of `fixtures/spec-a` (`[decision_records]`), with records made as `card_questions.rs` makes them. The test-engineer records the baseline texts of CC2 at 3892f03 first.

- [ ] CC1 — **The bundle `spec bundle` makes.** C-0001 has two targets T1, T2 and these items: a decision applied with record R1, an open question, a rejected decision. `spec card show C-0001 --context --budget 3000 --json` has `context.bundle` equal, as a JSON value, to `spec bundle T1 T2 R1 --budget 3000 --json`, with `refs` `[T1, T2, R1]`. The text ends with `context.bundle: <that hash>` and then that body, byte for byte. Without `--budget`, `budget` is 10000. With `[budgets] bundle_task = 1500` it is 1500, equal to `spec bundle … --budget 1500`. With `bundle_node = 900` alone it is 10000. A second read is byte-equal, and so is a copy at another root and `HOME` (text and JSON). Mutations, each red: the first target only; `bundle_node` read for the budget; records before targets.
- [ ] CC2 — **Not asked, nothing read.** Plain `show` and `--json` are the baseline plus the last line `context: -` and `"context": null`, every other key equal. `get_card {card_id}` is its baseline plus the same. Then every `.md` of the copy goes to mode 000 and `.git` is moved aside, after the card is made. `show C-0001` still exits 0 with no stderr line. With `--context` it exits 0 with the card's lines, `context.bundle` `null` and one `context:` note. Mutation, red: the context always built.
- [ ] CC3 — **The room.** (a) A card whose criteria make its lines about 25 000 characters, with three targets whose bundle at 10 000 would print more than the room. The text before the body plus `context.bundle.chars` stays ≤ 40 000, and `context.bundle.chars` ≤ 40 000 − U ("Text"). The body is shorter than `spec bundle`'s at the same budget alone. (b) A subtask with a 4 096 B goal, 32 criteria of 1 024 B and 64 targets, whose parent's goal is 4 096 B. It exits 0, the text stays within 40 000 characters (plus the truncated tail if cut), `context.bundle` is `null`, and there is one `no bundle:` note. Mutation, red: the bundle fitted against 40 000 instead of the room ((a) passes 40 000).
- [ ] CC4 — **Failures are notes.** On CC1's card, each case exits 0 with the card's document:
  - (a) T2's file deleted: `context.bundle` `null`, `context.notes` `["no bundle: <spec bundle's reason naming T2>"]`, stderr `note: context: no bundle: …`, and `get_card` not `isError`;
  - (b) `--budget 1`: the note names the minimum;
  - (c) `[budgets] bundle_task = 0`: a note naming its line, and with `--budget 3000` the bundle is made;
  - (d) a manual card: `no bundle: no targets and no recorded decisions`.

  These exit 2 with nothing read: `--budget 0`, `--budget 4294967296` and `--budget 5` without `--context`; MCP `budget` without `context` is `isError` with the library's line. Mutations, each red: the bundle's exit passed through ((a), (b)); the config error exit 2 ((c)).
- [ ] CC5 — **The parent's goal.**
  - A hand-off C-0002 (own goal G2) of C-0001 (goal G1): `context.parent` is `{id: "C-0001", title, goal: G1}`, with the lines `context.parent: C-0001 | <title>` and `context.parent.goal: G1`.
  - C-0001 itself: `parent: null`, `context.parent: -`, no note. A parent without a goal: `goal: null`, `-`.
  - The parent's row hand-corrupted: `parent: null`, the note `parent C-0001 not read: …`.
  - Its goal hand-edited to 5 000 bytes: cut within 4 096 bytes on a character boundary, with its note.

  Mutation, red: the card's own goal.
- [ ] CC6 — **Only the card's own applied records.** C-0001's items are a decision applied with R1, a question applied with R2, an open decision, a rejected one, and a discrepancy applied with no record. Its subtask C-0002 has a decision applied with R3, which shows in C-0001's `subtask_items`. C-0001's `refs` are its targets, then R1 and R2 by item ID. C-0002's include R3. The cap is a unit test of the selection (`#[cfg(test)]` in `card.rs`): 66 applied records give 64 REFs and the note `not bundled: 2 more records, from <the 65th's ID>`. Mutations, each red: open items taken; `subtask_items` taken; no cap.
- [ ] CC7 — **Parity, schemas, the hub.** The stdio `get_card {card_id, context: true, budget: 3000}` content is byte-equal to `spec card show C-0001 --marks --context --budget 3000 2>&1`, and its `structuredContent` equals the `--json`. The same holds with `history: true` and in CC4 (a). The schema walker passes with exact key sets: `context` `{parent, bundle, notes}`, `parent` `{id, title, goal}`, `bundle` the bundle document's 13. The input schema is `card_id`, `history`, `context`, `budget`, with `additionalProperties: false`. On a hub's `/mcp/<slug>` the same call answers alike, `tools/list` holds 18 tools, and `INSTRUCTIONS` is byte-equal to 3892f03's. Mutations, each red: a CLI key without its mirror (`notes` dropped from `CardContextShape`); `context` ignored on the hub's route.
- [ ] CC8 — **Plugin 0.39.0.** `plugin.json` is 0.39.0, `.env.example` `SPECENGINE_VERSION=0.39.0`, and `tag-check.sh v0.39.0` prints `0.39.0`. `PINS` and `SCHEMA_PINS` end with 0.39.0 entries. Mutation, red: the schema changed with no new `SCHEMA_PINS` entry (`tool_schemas_follow_the_plugin_version`).
- [ ] CC9 — **The texts.** `GET_DESCRIPTION` is as "Data" gives it: 1 987 B, ASCII, its numbers tied by asserts. `read-spec` has body 4 091 B and file 4 412, the sentence once in the cards bullet, the bullet's start and end kept, no digit, and its description unchanged. Stdio's `INSTRUCTIONS` is byte-equal to 3892f03's. Mutations, each red: `DEFAULT_TASK_BUDGET` set to 10 001 (the build fails); the sentence dropped.
- [ ] CC10 — **The daemon serves null; the UI types it.** `GET .../cards/C-0001` serves `context: null` among 35 keys. `fixtures/daemon-keys.json` `Card` names `context` after `history`, in 122 sets. `provisional.ts` `Card.context: null`, citing this criterion. UI lint has 0 problems, `tsc` is clean, and `pnpm test` is green, `daemonKeys.test.ts` included, with `AHEAD_OF_FIXTURE` empty at the end. Mutations, each red: the route asking `context: true`; `context` out of the fixture (`daemon_keys.rs`).
- [ ] CC11 — **[measure] Size and time.** `mcp_size` measures `get_card {context: true, history: true}` on CC3 (b)'s card and on CC3 (a)'s with 64 items, each with a 2 048 B `why` and a long summary. Text plus JSON stays ≤ 500 000 characters, and the maximum is pinned in the test and recorded in "Implementation". `--context` on a scratch copy of a real corpus: wall time warm, recorded.
- [ ] CC12 — **Gates.** `cargo nextest run` for `specengine-cli`, `-mcp`, `-http` is green, then the full run once. The eval tests (`doc_pointers`, `ui_policy`, `anonymity`) and `spec check` pass. UI lint, `tsc`, `pnpm test` and `pnpm build` pass, with first-load JS unchanged within 0.1 kB.

## Out of scope

An inline bundle for `get_task` and a subtask's records on its parent are out. So are any UI that shows a context, a new tool or `[budgets]` key, a context on `list_cards` or on the daemon's route, and a context cached or stored. A REF that does not resolve stays a known limit: it is never dropped alone while the rest is bundled.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-cli/src/{card,bundle,main,lib}.rs`; `crates/specengine-mcp/src/{cards,mirror}.rs`; `crates/specengine-http/src/app.rs` (one line); `plugin/specengine/.claude-plugin/plugin.json`; `plugin/specengine/skills/read-spec/SKILL.md`; `scripts/deploy/.env.example` |
| test-engineer, Rust | `card_context.rs` in CLI and MCP (new); `mcp_size.rs`; "Test churn"'s rows; `plugin_files.rs`, `plugin_skills.rs`, `plugin_roles.rs`; http `daemon_keys.rs`, `spec_ids_hub.rs`; `fixtures/daemon-keys.json` (`SPECENGINE_WRITE_DAEMON_KEYS=1`, once the Rust compiles), removing `["Card", "context"]` from `ui/src/api/daemonKeys.test.ts`'s `AHEAD_OF_FIXTURE` in the same change |
| ui-developer | "The UI line"'s four files, nothing else |

Comments cite `docs/features/card-context.md` "Acceptance criteria" with a criterion ID, or a canon heading; never "Data", which compaction drops (`doc_pointers`).

### Work order

1. The Rust test-engineer records CC2's baselines at 3892f03.
2. The ui-developer adds the UI line with its `AHEAD_OF_FIXTURE` entry; UI gates stay green. Meanwhile the rust-developer starts; the contract is fixed above.
3. The Rust test-engineer overlaps the Rust lane: CC1-CC6 in the CLI, then CC7-CC9, then the fixture (CC10, removing the entry) and CC11, with the churn. Then the Rust review.
4. Ship: the spec-writer ("Canon at ship").

The UI lane (`canvas-clicks`) runs beside it, sharing only documents; the owner commits each slice on its own. At most three iterations a lane.

### Risks

- **The ceiling arithmetic** (CC3): a ceiling forgotten in one path prints over 40 000, and Claude Code moves the answer into a file.
- **Room left:** `read-spec` 4 091 of 4 096, the description 1 987 of 2 048.
- **Cost:** a context adds a subset bundle, 0.37 s at 100 000 nodes (08 step 12d), inside the project's turn on a hub.

### Canon at ship

Free bytes now are in brackets. "Net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/card-context.md` (new, Tier 2, the `card*` shard) | the rule, the text and JSON, the room, the budget, the notes, the hub and the daemon's `null`, Known limits (a gone REF nulls the bundle; live text; older servers), Tests |
| `docs/canon/cards.md` [67] | "Commands": `show`'s `[--context [--budget N]]`; "MCP": `get_card`'s `context?`, `budget?`; "Documents and events": `context` after `history`, a pointer (net not positive) |
| `docs/canon/spec-cli-bundle.md` [2] | "Fitting": a crate-internal ceiling below 40 000 for a card's room, a pointer (net not positive) |
| `docs/canon/mcp-read.md` [12] | "Size": `get_card` with `context`, its measured maximum (net not positive) |
| `docs/canon/plugin-files.md` | "Changing it", **Version**: current 0.39.0; 0.39.0 `get_card`'s `context` returned, `context` and `budget` taken (with `read-spec`'s card sentence) |
| `docs/canon/hub-deploy.md` [12], `docs/canon/plugin.md` [316]; crate READMEs (cli [1], mcp [7]) | the version they name (net zero); a README only where it names `show`'s flags |
| `CLAUDE.md`; 08 s2 Phase 7 "Owner's word, 2026-11-12" row 1; RM-0108 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner did not answer: D1 to D10.

**The spec-writer's:** W1 the text: `context: -`, or the `context.*` lines with the body last, so "a line per document key" holds; W2 the `context.bundle:` line holds the hash alone, 84 characters, so the room is known before the fit; W3 the bundle's messages pass through; W4 a bad `bundle_task` is a note, never exit 2 as in a package; W5 at most 64 records; W6 the parent's goal cut only after a hand edit; W7 `budget` without `context` refused, never ignored; W8 `BundleRequest` unchanged, the ceiling internal; W9 the UI line first, behind `AHEAD_OF_FIXTURE`, so no gate goes red between the lanes.

**The owner's questions:**
- **Q-A.** Inline or by reference? Working answer: inline and opt-in. By reference keeps two calls, so his wish goes unmet.
- **Q-B.** Into `v5`? Answered by his word of 2026-11-12: the first core slice.
- **Q-C.** Which budget? Working answer: `bundle_task`. A new `bundle_card` key would likely make an older `spec check` refuse an unknown `[budgets]` key (unverified).
- **Q-D.** A subtask's records on its parent? Working answer: no; their IDs are in `subtask_items`. Taking them adds budget contention and size.
- **Q-E.** A gone REF: null the whole bundle, or drop it and bundle the rest? Working answer: null, with a note (one fitting, one hash, the same answer as `spec bundle`). Dropping it costs a resolve pass before the bundle, or a retry per gone REF.
