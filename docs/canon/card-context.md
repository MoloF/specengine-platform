---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, plugin, ui]
owner: owner
reviewed: 2026-11-12
---

# Card context: a task read with its context in one call

Built by `card-context` (RM-0108, plugin 0.39.0) on accepted decisions alone: a package carries the bundle within budget (ADR-0027), a card is a light record that reads (ADR-0056), reads never fail on the code layer (ADR-0046), nothing is blocked (ADR-0012). Related pages: the card, `cards.md`; the bundle, `spec-cli-bundle.md`; sizes, `mcp-read.md` "Size". Code: CLI `card.rs` (`add_context`, `context_{parent,refs,budget,room,bundle}`), `bundle.rs` (`bundle_within`); MCP `cards.rs`, `mirror.rs`; http `app.rs`.

## The rule

`spec card show C [--history] [--marks] [--context [--budget N]]`; MCP `get_card {card_id, history?, context?, budget?}`, its content byte-equal to `show --marks … 2>&1` (`mcp-read.md` "Tools", **Parity**). Opt-in: unasked, `context` is `null`, the text's last line `context: -`, and the read reads no parent, no config for a budget and no bundle, so plain `show` stays a queue read. Asked, on any card (cancelled, accepted, stranded too), `context` is `{parent, bundle, notes}`, every part present. No new tool, `[budgets]` key, schema step, backup format or route. The context decides nothing: no move, accept, gate or rollup reads it.

- **REFs**: the card's `targets` as stored (canonical, at most 64), then the `record_id` of each of its own items `applied` with one, of any kind, in `items` order, at most 64 (`NODES_MAX`), past them a note. One fitting, one `bundle_hash`. A subtask's records stay out of its parent's bundle: their IDs are in `subtask_items`. Passed as they are, never de-duplicated: the bundle counts REFs naming one node once.
- **Budget**: `--budget N` (1 to 4294967295, `spec bundle`'s `budget_of`), else the reading root's `[budgets] bundle_task`, read alone as a package's, else `DEFAULT_TASK_BUDGET` 10 000. It goes as `BundleRequest.budget = Some(n)`, so `bundle_node` is never read and a minimum's line names `--budget N`.
- **Parent**: the direct parent's `{id, title, goal}`, read now from the same queue; none: `null`, no note. Unreadable: `null` and a note. A goal over `GOAL_MAX` (4 096 B, a hand edit) is cut at the last character boundary within it, with a note.
- **Live text**: the bundle reads the corpus now, never the card's `snapshot`; the card's `mark` says when its brief changed.

## The room

`U` is the sum, over every line the text prints before the body, of its characters + 1; the `context.bundle:` line counts at its made length, 84 (`BUNDLE_LINE_CHARS`). The bundle is fitted by the crate-internal `bundle_within(env, globals, request, chars_max)` with `chars_max = OUTPUT_CAP_CHARS − U`, so the text before escaping stays within 40 000; `U ≥ 40 000`, the card's lines cut included, asks no bundle (`no room`). Claude Code moves answers above 48 000-60 000 characters into a file (`mcp-read.md` "Owner's check"), so 40 000 binds. Every other bundle keeps `OUTPUT_CAP_CHARS` (`spec-cli-bundle.md` "Fitting"). Probes: `U` 39 724 leaves 276, 39 993 leaves 7.

## Text and JSON

After `history`, the bundle's body last:

```text
context.parent: C-0001 | Write the tide table foreword
context.parent.goal: Readers learn the tides before the tables.
context.bundle: b3:<64 hex digits>
# Bundle: MEC-TIDE-TABLES, DEC-0031
<the body, as `spec bundle` prints it, without its two closing lines>
```

`context.parent:` is `<id> | <title>` or `-`; `context.parent.goal:` the goal through `one_line`, or `-` (no parent, no goal); `context.bundle:` the `bundle_hash` or `-`, the body only when made. Tokens, characters and bytes are in the JSON. The context's notes print as `note: context: <note>` after every message the read printed before, then the bundle's own `note:` and `warning:` lines verbatim, all outside the cut. The whole text, body included, goes through `escape_controls`.

JSON: `context` is the card document's 35th key, between `history` and `notes` (`cards.md` "Documents and events"): `null`, or `{parent: {id, title, goal} | null, bundle, notes}`. `bundle` is `spec bundle --json`'s document exactly, 13 keys (`spec-cli-bundle.md` "Output"), from the bundle's own serializer, equal as a value to `spec bundle <REFs> --budget N --json`; never its exit-1 document. `notes` drop the `context: ` prefix; the document's top-level `notes` hold none of them.

Library: `CardShowRequest` has `context: bool`, `budget: Option<i64>`, set at every construction; `CardShowOutcome.context: Option<CardContext>`; `CardContext {parent: Option<CardContextParent>, bundle: Option<BundleOutcome>, notes}`, `CardContextParent {id, title, goal}`, both exported. `BundleRequest` and `bundle()` are unchanged.

## Notes and exits

The notes, in this order; from the fourth row on, `bundle` is `null`:

| When | `context.notes` entry |
|---|---|
| the parent does not read | `parent C-0001 not read: no card C-0001`, or the queue's corrupt-row line |
| its goal over 4 096 B | `parent C-0001's goal cut at 4096 bytes` |
| more than 64 records | `not bundled: <k> more records, from <first left out>` |
| no targets, no records | `no bundle: no targets and no recorded decision` |
| `U ≥ 40 000` | `no bundle: the card's lines leave no room under the 40000-character ceiling` |
| a bad `bundle_task` | `no bundle: <the config error, at its line>` |
| `spec bundle` exit 1 | `no bundle: <its reason>`: a gone REF, named |
| `spec bundle` exit 2: the minimum, the frame over the room, `HOME`, store, index | `no bundle: <its line, its "spec: " prefix dropped as a package's, one_line>` |

Exits are as before: 0 answered, a context with notes included; 1 no such card, no context. Exit 2 adds, before anything is read, `` `--budget` needs `--context` `` (the library's line, no clap rule, so CLI and MCP print it alike), then `--budget` 0 or past 4294967295 (`budget_of`'s line). MCP: `context: true` asks, absent, `null` or `false` do not; `budget` without it is `isError`, no `structuredContent`.

Writes: the data directory alone (the bundle's index update and code refresh), nothing under the root, no queue row, no event. One queue and corpus state gives one text and one JSON, byte-equal across roots and `HOME`s.

## MCP, daemon, hub, plugin

- `GetCardArgs` takes `context`, `budget` (`deny_unknown_fields`). The output schema mirrors the document: `CardContextShape {parent, bundle: BundleDocument, notes}`, `CardContextParentShape {id, title, goal}`, both `deny_unknown_fields`. `GET_DESCRIPTION` is 2 041 of 2 048 B since `upstream` (`card-upstream.md`), its "else 10000)" and "the 40000 characters" tied to `DEFAULT_TASK_BUDGET` and `OUTPUT_CAP_CHARS` by `holds_number` asserts. Stdio's `INSTRUCTIONS` is unchanged.
- The daemon's `GET .../cards/:id` never asks: `context: null`, `Card` 36 keys (`fixtures/daemon-keys.json`, 137 sets). The UI types `Card.context: null` (`provisional.ts`) and renders nothing.
- A hub's `/mcp/<slug>` `get_card` is stdio's route as it is: it takes the key with no hub code, the bundle reading the hub's checkout inside the project's turn (`hub-mcp.md` "Turn"); its 18 tools and `INSTRUCTIONS` unchanged. A hub gains it with its next upgrade, the owner's.
- Plugin 0.39.0 (`plugin-files.md` "Changing it"): `read-spec`'s cards bullet names `context: true`, and `get_context_bundle` for an older server.

## Cost

`get_card {context, history}` at most 432 982 characters, text and JSON, of `MAX_RESULT_CHARS` 500 000 (`mcp_size`'s `CARD_CONTEXT_MAXIMUM`: 64 items, the card a subtask whose `upstream` lists 16 edits by a 128-byte name, its context the parent's 4 096-byte goal; 424 208 before `upstream`); a 64-target subtask 96 131. A scratch copy of a 375-document corpus, debug, warm, 3 targets: `show --context` 0.48-0.51 s, plain `show` 0.01 s, `spec bundle` of the same REFs 0.47-0.50 s.

## Known limits

- **A gone REF** (a deleted target or record) nulls the whole bundle, never only itself; the note names it, and the agent bundles the rest with `get_context_bundle`.
- **Escaping.** `escape_controls` runs over the body too, so on a corpus holding C0/C1 controls or bidi marks the body's bytes differ from `spec bundle`'s and `get_context_bundle`'s; and escaping can pass 40 000, the residue `mcp-read.md` "Size" names.
- **The frame over the room** reads `… over the <room>-character ceiling, whatever the budget (--budget N) …: name fewer targets`: it names `--budget` when none was passed, and the room, not the budget, limits it. To fold into the `no room` note.
- **`GetCardArgs.budget`'s doc** names 4294967295 and 10000 with no assert.
- **Older servers**, a terminal's or a hub's at 0.38, refuse `context` (`unknown field`); `read-spec` names the fallback.

## Tests

CLI `card_context.rs` (17; CC2's baselines `card_context_before.json`, from 82a7d39's binary) and `card.rs`' `context_tests` (the 64-record cap); MCP `card_context.rs` (5: parity, schemas, failures, the hub, the texts), `mcp_size` (`card_context_stays_under_the_declared_cap`); http `cards.rs` (`null`), `daemon_keys.rs`; `plugin_{files,skills,roles}.rs`; over scratch git copies of `fixtures/spec-a`. Gaps: the hub's call runs in-process, not over HTTP; no record that is also a target; an applied deciding item with no record cannot be stored (it reads as a corrupt row), so CC6's case is a row set in SQL and the unit test; no MCP golden of its own for CC2; `escape_controls` over a body with control characters.
