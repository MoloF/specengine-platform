---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-11
---

# Card split: a task and a subtask per department, in one act

ADR-0078, built by `department-split` (RM-0100, plugin 0.37.0). The card: `cards.md`; a hand-off and the new-card rule: `card-flows.md`; departments and their disciplines: `workspaces.md`; a parent's progress: `card-progress.md`.

<a id="the-rule"></a>
## The rule

A split hands one piece of work to several departments at once. It makes a parent task on a document, or takes a stored task as the parent, and one subtask per chosen discipline: each a hand-off (`card-flows.md` "The rule"), its `parent` the parent, its `reviewer` the split's (named, else the parent's, else the acting name), its flow and first column the new-card rule's for its discipline. It is one write: one transaction, all cards or none, one `card.created` each, the parent first. It writes no stored card: a parent that exists keeps its row, revision and history. So no other card write creates, moves or refuses another, and a split makes no card wait on another (ADR-0074).

A `[[card.split]]` row under `[card]` names a node kind and the disciplines its work usually needs. Rows only pre-select: the disciplines of the rows whose kind a node of the parent's targets, or a node under them, holds; and a subtask's targets, the nodes of its rows' kinds found there, else the parent's targets. A row never refuses, requires or orders a split, and no move, accept, rollup, gate or check reads one (ADR-0012). Kinds and disciplines are the project's words; the core names none (ADR-0008). ADR-0078.

## Config

`[card]` holds `flow` and `split`; a row's keys `kind`, `disciplines`. Core `card_from_toml` reads the rows with the flows into `CardConfig.splits` (`CardSplit {kind, disciplines}`, config order, empty without the key):

```toml
[[card.split]]
kind = "broadsheet"
disciplines = ["copy", "plates"]
```

- `kind`: one row per kind, `directions.md` "Config"'s checks (1-64 bytes, no control character, no end space), matched byte for byte, never checked against `[ids]`.
- `disciplines`: 1-32 (`MAX_SPLIT_DISCIPLINES`) through the flows' list reader (`name_problem`, each once), each listed by some `[[card.flow]]`; `PLAIN_FLOW` lists none, so under it no row reads. At most 64 rows (`MAX_SPLITS`); an empty array is no key.

A bad row is a bad `[card]` (`card-flows.md` "Config"): `spec card` exits 2, the card routes answer 503, the workspaces read serves `disciplines` empty with a note; nothing else changes. The listing check runs once every flow reads, so a flow's own error stands alone. Messages mirror `[[card.flow]]`'s (`` unknown field `<x>`, expected `flow` or `split` ``, the array's and a row's type, the 65th table, a missing `kind` or `disciplines` at the table's line), a kind's as a direction's prefixed `` `kind`: ``, and: `` `[[card.split]]` kind `<k>` is written twice ``; `` `disciplines` lists none; a row lists 1 to 32 `` (`lists <n>` past 32); `` `[[card.split]]` `<k>`: no `[[card.flow]]` lists the discipline `<d>` ``.

## The plan

Core `split_plan(config, scope, parent_targets, chosen)`, pure, its `Err` a refusal's reason:

1. **Scope**, built by the CLI: per parent target in stored order, the rows `spec tree <target> --depth 2` serves (target first), each node once at its first place, named as `resolved_list` stores a target (its ID, else its document's path); an ID-less section passed over; no kind, `None` (`ScopeNode {target, kind}`).
2. **Matched rows**: `kind` equal to a scope node's, config order; `None` matches none.
3. **`suggested`**: their disciplines, row then list order, each once.
4. **Children**: `chosen` as written; `None` (a dry run, no `--child`) every discipline the flows list, flow then list order. Refused: `child: none named; name 1 to 32`, `child: <n> named; a split makes 1 to 32` (`MAX_SPLIT_CHILDREN`), `` child: `<d>` is named twice ``, `child: <problem>`.
5. **Each child** (`PlannedChild`): `flow_for(Some(d))`, its `Err` prefixed `child: `; `flow_title` the flow's title, else name; its first column. Targets: the scope nodes of the kinds of the matched rows listing `d`, scope order, `from` `kinds`; past 64 the first 64 and `` subtask `<d>`: <n> nodes of its kinds; the first 64 taken ``; none, the parent's targets, `from` `parent`. `suggested`: `d` in step 3.

## Command

```text
spec card split --target REF... --title T [--goal T] [--discipline D] [--child D]...
  [--reviewer N] [--as N] [--dry-run]
spec card split --parent C [--child D]... [--reviewer N] [--as N] [--dry-run]
```

`card_split(&Env, &Globals, &CardSplitRequest)` -> `CardSplitOutcome`; the daemon's `CardSplitBody`. Checks in order, each stopping:
1. **Usage**, exit 2: `--parent` with `--target`, `--title`, `--goal` or `--discipline`; no `--parent` and no `--title`; no `--child` without `--dry-run` (``name each subtask's discipline with --child, or plan with --dry-run``); `--as`; a look-alike `--parent`; `C` in `[ids]`. Then a bad `[card]`, exit 2.
2. Exit 1: `field_problem`, `discipline: <problem>`; the stored parent, in any state (`parent: no card C-0099`); a write's reviewer: named, else the stored parent's, else the acting name, then `assign`'s grammar and `reviewer_problem` (`agent`, `local` refused); `--target` by `resolved_targets` and the scope read; the plan, then a new parent's `flow_for`; the store.

A dry run stops before the store and judges no person. The flag's name is `SPLIT_DRY_RUN_FLAG`: `proposal_genre`'s guard bans the literal in `card.rs`.

**Store** `SqliteQueue::split_cards(&NewSplit, now)` -> `SplitCards {parent, created, children}`, one `Immediate` transaction: `SplitParent::Stored(id)` read by `select_card` (none: `CardRefused`), `SplitParent::New {card: Box<NewCard>, snapshot}` by `insert_card`; each child's `parent` set inside it, then `insert_card`, reviewer and links asked again under the lock; no children, `child: none named`. A refusal rolls all back: no row, no event, no ID taken. No storage change: queue schema 11, backup format 5, `card.created` as ever; a split's card is told by its `parent` alone.

## The document

`--json`, `split_card`'s `structuredContent`, the route's 200 and 409; every key, in order (`fixtures/daemon-keys.json` `CardSplit`, `CardSplitParent`, `CardSplitChild`):

```json
{"dry_run": false,
 "parent": {"id": "C-0001", "created": true, "title": "The weekly run", "discipline": "copy",
   "flow": "copy", "flow_title": "Copy desk", "column": "draft", "targets": ["PRN-WEEKLY"]},
 "children": [{"id": "C-0002", "discipline": "copy", "flow": "copy", "flow_title": "Copy desk",
   "column": "draft", "targets": ["SHT-BACK"], "from": "kinds", "suggested": true}],
 "reviewer": "Ana Ruiz", "notes": []}
```

A stored parent: `created` false, its stored fields. A dry run: every `id` null, a stored parent's too; `reviewer` the chain's, unjudged. A refusal (exit 1, a 409): `parent` null, `children` `[]`, `reviewer` null; `notes` the reason alone before the plan, after it the plan's notes then the reason. Text, a line each, `-` for none:

```text
split C-0001 new: copy/draft | The weekly run | PRN-WEEKLY | reviewer Ana Ruiz
subtask C-0002: copy/draft | copy | SHT-BACK, SHT-FRONT, SHT-INNER | by kind, suggested
note: <note>
```

`stored` for a stored parent; `the task's targets` for `by kind`; a dry run `-` per ID, `plan` for `split`, ending `dry run: nothing written`.

## Route and hub

`POST /api/projects/:p/cards/split`, `CardSplitBody` `{target, title, goal, discipline, parent, child, reviewer, dry_run, as}`, `deny_unknown_fields`, each optional, then the CLI's usage rules; `child` null or `[]` none, `dry_run` null false. `board-ui.md` "The door", then one `card_split` (the twelfth of `CARD_CALLS`, handler `split_card`) in the project's turn, one clock reading. 200 the document, written or planned; 400 a body, person or usage rule; 409 refused, the document; 503 cannot run. No 404: a missing `parent` is a 409. Else 405 `Allow: POST`. `split` is a static segment. A written split adds one `card.created` per card, a dry run none. **A hub**: the seventh plain write, any login, `as` bound to it, a dry run too (`hub-team.md` "Classes").

## MCP

`split_card` on stdio (30 tools) and a hub's `/mcp` (18): `SplitCardArgs`, `deny_unknown_fields`, `target`, `title`, `goal`, `discipline`, `parent`, `child`, `reviewer`, `dry_run`, each nullable, no `as`. Stdio acts as `agent`, so it names a person reviewer or splits a card that has one; a hub as the login, its call walking. Output: the document; a refusal an error result with the reason and the document. Texts ASCII, under 2 048 B, the determinism sentence: stdio 1 655 B, hub 1 670 ("You act as your login"). `INSTRUCTIONS`' and `HUB_INSTRUCTIONS`' cards lines name it (`mcp-read.md` "Tools", `hub-mcp.md` "Tools"); `read-spec` too.

## Screen

"Split to departments…" opens `SplitDialog`, one lazy chunk, never preloaded: on the document modal and page after New task (`{target: [REF], title}`); on Flow's panel, the rooted scope and a box, after New task; on a task's work item after Hand off, open cards alone (`{parent: C}`, only subtasks made).
- **The plan read**, once on open: `dry_run: true`, `child: null` (`useSplitPlan`, held for the dialog: `staleTime` Infinity, `gcTime` 0); a skeleton; a failure's words and Retry. A form change reads nothing.
- **The form.** A new parent's Title (prefilled), Goal, Discipline (the department's first); Reviewer as Hand off's (`handOffReviewer`; a hub's login, read-only); Subtasks, a checkbox per plan child, `suggested` checked, grouped under the first department whose `flows` names the row's flow, the rest last: its `flow_title`, the raw discipline when it differs, "By kind: <n> documents" or "The task's targets", the targets in a `Disclosure`. From a document, "Tasks on this document" from the held trace read under its REF (`heldTraceRef`).
- **Split** posts the checked disciplines in row order (`useCardSplit`, the card writes' key and re-reads): 200 closes with "Split into C-0012 and 3 subtasks" ("3 subtasks for C-0012" from a task); 409, 400 the reason in the alert, the fields kept; 503 the daemon's words.

The page computes no target, suggestion or placement. Words `board.split.*`. Mock (harbor-sim): rows `mechanic` `[copy, plates]`, `rule` `[plates]`; `?scenario=conflict` refuses the first write.

## Known limits

- **Older binaries** refuse `split` (`` unknown field `split`, expected `flow` ``), their card surfaces alone (`card-flows.md` "Known limits"): commit a row only once every terminal, plugin and hub serving the project reads it. The preset's rows stay commented.
- **A typo in a row** stops this build's card surfaces too; `spec card list` names its line, `spec check` does not.
- **No dedup**: two splits make two sets; the dialog lists the document's tasks. A split names cards of one store.
- **A stored parent is read outside the transaction**: a concurrent edit can leave its children the old title or goal.
- **A dry run's reviewer**: the route judges its format (`persons_named`), the CLI does not.
- **A duplicated ID**: a target two documents carry walks both, its row keeping the first's kind.
- **Cost**: each split or dry run decodes the whole index, a graph and a resolver; 0.25-0.39 s on a real game corpus (R1, 2026-11-11).
- Subtasks carry the parent's title; the Board tells them apart by department (the owner's Q-C open).

## Tests

core `card_split.rs` (18: config, plan); store `card_split.rs` (5: all or none); CLI `card_split.rs` (15, `fixtures/card-press`'s two live rows: split, stored parent, dry run, no cascade); http `card_split.rs` (7: route, hub), `door.rs` (`CARD_CALLS` 12, three `card.created`), `hub.rs` units (`class`), `daemon_keys.rs` (121); mcp `card_split.rs` (5), `mcp_hub.rs` (18); `plugin_files.rs`, `plugin_skills.rs`. `no_domain.rs` takes each row's `disciplines` as words, and its `kind = "…"` scan reads live lines alone, a TOML comment line no config. UI `board/SplitDialog.test.tsx`, `mocks/split.smoke.test.tsx`, `MockClient.test.ts`.
