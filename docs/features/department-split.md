---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, ui, plugin, scripts, fixtures]
ref: RM-0100 re-scoped, v5 step 6c (08 s2 Phase 7, "Owner's vision, 2026-11-11"); the analyst's requirement analysis of 2026-11-11 ("SPLIT", S1-S9, U1-U3); the orchestrator's working answers, the owner's review open
adrs: [ADR-0078]
---

# department-split: a task and a subtask per department, in one act (RM-0100)

## Why

The owner's target workflow of 2026-11-11 (`08-roadmap.md` Phase 7, "Owner's vision, 2026-11-11"): a writer describes the game; it is split into subtasks per department; questions go back up; each subtask is binding, or its department decides. His draft case is "a new female character": narrative, art and sound each get a subtask, and the sound designer picks the voice.

Today:
- **A split takes several acts.** New task on the document, then one Hand off per department, each a separate write with its own dialog (`card-flows.md` "Hand-off"). A writer who forgets one department leaves a hole nobody sees.
- **Nothing says which departments a document needs.** The project knows its kinds (a character, a scene) and its departments' disciplines, but no config ties them.
- **Each hand-off targets what the person types.** A sound subtask on a scene should target the scene's sound nodes, not the whole scene, when the scene holds them.
- **An agent cannot split.** `create_card` makes one card a call.

This slice makes the split one act: a parent task and one subtask per chosen discipline, all or nothing, from the document, Flow's panel or a task, by the CLI, MCP and the daemon. `[[card.split]]` rows let a project say which disciplines a kind usually needs, and which nodes each subtask targets; they only pre-select. ADR-0078 supplements ADR-0056, ADR-0074 and ADR-0076; its canon is `docs/canon/card-split.md` "The rule", and `card-flows.md` "The rule" names the split as the one act that creates several cards. The mandate (binding, or the department decides) is the next slice, RM-0104 `task-mandate`.

## Description and interactions

### Decisions

- **D1. One act.** `spec card split`, `split_card` and `POST .../cards/split` make one parent card on targets, or take a stored card as the parent, and one child per chosen discipline. A child's `parent` is the parent; its `reviewer` the split's reviewer (D3); its `title` and `goal` the parent's; its targets the plan's (D5); no assignee, criteria, estimate, due or evidence. Each card lands last in the first column of the flow the new-card rule picks for its discipline (`card-flows.md` "Config"), a new parent by its own `discipline`.
- **D2. All or none.** One `Immediate` transaction in the store: the new parent first, then the children in the order asked; one `card.created` each, its payload `new`'s (`cards.md` "Documents and events"). Any refusal rolls everything back: no row, no event, no ID taken. A stored parent is read, never written: its row, `revision`, `updated_at` and history stay as they are.
- **D3. The reviewer**: `reviewer` given, else the stored parent's, else the acting name; then `assign`'s person grammar and core `reviewer_problem`, so `agent` and `local` are refused with today's lines. A new parent takes it too. So a tool on stdio, acting as `agent`, names a person or splits a card that has one; a hub's login is a person.
- **D4. Templates.** `[[card.split]]` rows under `[card]`, each a `kind` and its `disciplines` ("Data"). Every discipline a row lists must be one some `[[card.flow]]` lists, else `[card]` does not read, as any bad `[card]` (`card-flows.md` "Config"). A row only pre-selects and picks targets (D5): it never refuses, requires or orders a split, and nothing else reads it (ADR-0012).
- **D5. The plan**, core `split_plan`, pure. The scope is each parent target with the nodes under it to depth 2 (the tree read's rows, as a document's Inside: `doc-view.md` "Inside"). A row matches when its `kind` equals a scope node's `kind` byte for byte. `suggested`: the matched rows' disciplines. A child's targets: the scope nodes of the kinds its matched rows name, at most 64; none found, the parent's targets ("Data").
- **D6. `--dry-run`** serves the plan and writes nothing. It judges every rule of the write but the persons (`--reviewer`, `--as`). With no `--child` it plans every discipline the flows list, each with `suggested`; the dialog shows that plan.
- **D7. The daemon.** `POST /api/projects/:p/cards/split`, one library call `card_split`, the twelfth of `CARD_CALLS`, handler `split_card`. On a hub it is the seventh plain write: `hub.rs` `class` lists it, `hub::acting` binds `as` to the login, `[team]` is not read.
- **D8. MCP `split_card`**, stdio and a hub's `/mcp`: 29 tools become 30, a hub's 17 become 18. Stdio acts as `agent`, a hub as the login. A write, so a hub's call walks (`hub-mcp.md`).
- **D9. The word is "split".** Flow's "cuts" stay Flow's. The chrome: "Split to departments…", never a trade's word.
- **D10. The dialog**: one lazy chunk, `SplitDialog`, opened from the document modal and page (after New task), Flow's panel (the scope and a box, after New task) and a task's work item (after Hand off: the task is the parent, only subtasks are made).
- **D11. The preset** gains a `sound` flow and a `sound` department, two commented `[[card.split]]` rows, and a header line naming the key; `studio-lamp` keeps its verbatim copy; `card-press`, the non-game fixture, carries two live rows.
- **D12. No storage change.** Queue schema 11, `CARD_COLUMNS` 25, backup format 5, `QUEUE_EVENT_TYPES` 25 and `card.created`'s payload all stay. A card made by a split is told only by its `parent`.
- **D13. ADR-0074 holds** for every other write: no write on a card creates, moves or refuses another; no card waits on another (ADR-0012). The parent's progress counts the children as any (`card-progress.md`).
- **D14. Plugin 0.37.0**: a new tool; `read-spec` names it.
- **D15. Rollout.** `[[card.split]]` makes an older binary fail its card surfaces alone; rows go in only after every terminal, plugin and hub serving the project runs this build ("Rules and edge cases", Rollout).

### The contract (Rust lane)

- **Core `card_toml.rs`.** `CARD_KEYS` `["flow", "split"]`; `SPLIT_KEYS` `["kind", "disciplines"]`; `MAX_SPLITS` 64; `CardSplit {kind: String, disciplines: Vec<String>}`; `CardConfig.splits: Vec<CardSplit>`, config order, empty without the key. The kind's four checks are `directions`' (`MAX_KIND_BYTES` 64, no control character, no end space; `directions.md` "Config"), shared, not copied; `disciplines` through the flows' list reader (`name_problem`, each once, at most 32), then at least one, then each listed by a flow. Messages: "Data".
- **Core `card_split.rs`** (new, pure). `ScopeNode {target: String, kind: Option<String>}`; `TargetsFrom::{Kinds, Parent}` (serialized `kinds`, `parent`); `PlannedChild {discipline, flow, flow_title, column, targets, from, suggested}`; `SplitPlan {suggested: Vec<String>, children: Vec<PlannedChild>, notes: Vec<String>}`; `split_plan(config: &CardConfig, scope: &[ScopeNode], parent_targets: &[String], chosen: Option<&[String]>) -> Result<SplitPlan, String>`; `MAX_SPLIT_CHILDREN` 32. The `Err` is a refusal's reason ("Data").
- **Store `queue/cards.rs`.** `SplitParent::{New {card: NewCard, snapshot: Vec<CardFrozen>}, Stored(String)}`; `NewSplitChild {card: NewCard, snapshot: Vec<CardFrozen>}`; `NewSplit {parent: SplitParent, children: Vec<NewSplitChild>}`; `SqliteQueue::split_cards(&mut self, split: &NewSplit, now: &str) -> Result<SplitCards, QueueError>`; `SplitCards {parent: Card, created: bool, children: Vec<Card>}`. One `self.write()` transaction: `Stored` read with `select_card` (none: `CardRefused`, `parent: no card C-0099`), `New` through `insert_card`; each child's `parent` set to the parent's ID inside the transaction, whatever the input held, then `insert_card` (reviewer and links asked again under the lock); one commit. An empty `children` is `CardRefused` (`child: none named`). Snapshot empty means no snapshot, as `create_card` stores it.
- **CLI `card.rs`, `main.rs`.** `CardSplitRequest {targets, title, goal, discipline, parent, children: Option<Vec<String>>, reviewer, dry_run, actor, now}`; `card_split(&Env, &Globals, &CardSplitRequest) -> Result<CardSplitOutcome, CliError>`; `CardSplitOutcome` the document and its exit; `CardSplitBody` (`deny_unknown_fields`), `CardSplitBody::from_json`, `::request(now)`. The scope read: per parent target, the subset tree read `spec tree <target> --depth 2` serves. `card split` in `main.rs` ("Data").
- **MCP `cards.rs`, `hub.rs`, `mirror.rs`.** `SplitCardArgs` (`deny_unknown_fields`), `split_card` on the stdio server and in `HUB_TOOLS` (18), each with its own text ("Data"); `mirror.rs` `CardSplitDocument`, `CardSplitParentShape`, `CardSplitChildShape`.
- **HTTP `app.rs`, `hub.rs`.** The route, its handler `split_card` (no `card_` prefix), `class`'s arm `["cards", "split"] if post => Some(Class::Plain)`, the answers of "Data".
- **Plugin** 0.37.0 (`plugin.json`, `PINS`, `SCHEMA_PINS`, `scripts/deploy/.env.example`, `spec_ids_hub.rs`, `tag-check.sh`); `skills/read-spec/SKILL.md` as "Data".
- **Preset** `scripts/presets/gamedev.toml` as "Data".

### The view (UI lane)

- **Types** (`src/api/provisional.ts`): `CardSplitBody`, `CardSplit`, `CardSplitParent`, `CardSplitChild`. `daemonKeys.test.ts` lists the three sets in `AHEAD_OF_FIXTURE` until the Rust lane's `fixtures/daemon-keys.json` lands; empty at ship.
- **Client** (`src/api/client.ts`, `http.ts`, `MockClient`): `splitCard(project, body)`, the answer a `CardSplit`. The plan read (`dry_run: true`) is a query, `useSplitPlan`, keyed by project and source, held for the dialog's life, outside the card writes' mutation key; the write is a mutation sharing `useCardWriting`'s key and the card writes' re-reads.
- **The dialog**: `src/board/SplitDialog.tsx` (new), loaded by `lazy()` at each opener and rendered only when open, never preloaded ("Data", The dialog).
- **Openers**: `src/doc/DocBody.tsx` (`DocActs`), `src/flow/FlowView.tsx` (the panel's scope and box), `src/board/CardActs.tsx` (after Hand off).
- **Words** (`src/i18n/en.ts`, `ru.ts`): "Data", Catalog words.
- **Mocks** (`src/mocks/cards.ts`, `MockClient`): "Data", Mocks.

## Data

### `[[card.split]]`

Under `[card]`, beside `[[card.flow]]`; `card-press` holds these two:

```toml
[[card.split]]
kind = "broadsheet"
disciplines = ["copy", "plates"]

[[card.split]]
kind = "print-run"
disciplines = ["plates"]
```

- `kind`: one node kind, 1-64 bytes, no control character, no space at an end, one row per kind; matched exactly, never checked against `[ids]`.
- `disciplines`: 1-32 names, `name_problem`'s grammar, each once, each listed by some `[[card.flow]]`'s `disciplines`. A flow listing none takes no row's discipline: under `PLAIN_FLOW` no row reads.
- At most 64 rows. An empty array is no key.

Read by core `card_from_toml` with the flows, first error by line, `<config>:<line>: <message>`: `spec card` exits 2, the card routes answer 503, the workspaces read serves its `disciplines` empty with one note (`workspaces.md` "Config"), and no other command, finding or gate changes. The listing check runs once every flow reads; a flow's own error stands alone.

```text
unknown field `<x>`, expected `flow` or `split`                         at the key ([card])
`[card] split` is an array of tables (`[[card.split]]`), not <a type>    at the value
at most 64 `[[card.split]]` tables                                       at the 65th
`[[card.split]]` is a table, not <a type>                                at the entry
unknown field `<x>`, expected `kind` or `disciplines`                    at the key
`[[card.split]]` has no `kind`                                           at the table's line
`[[card.split]]` `<k>` has no `disciplines`                              at the table's line
`kind` is a string, not <a type>                                         at the value
`kind`: `<k>` is <N> bytes long; a kind has 1 to 64                       at the value
`kind`: a kind holds a control character                                 at the value
`kind`: `<k>` has a space at an end                                      at the value
`[[card.split]]` kind `<k>` is written twice                              at the second's `kind`
`disciplines` lists none; a row lists 1 to 32                            at the value
`[[card.split]]` `<k>`: no `[[card.flow]]` lists the discipline `<d>`      at the entry
```

`disciplines`' type, grammar, repeat and 33rd-entry messages are the flows' list reader's for `disciplines`.

### The plan

Core `split_plan`, in order:

1. **Scope** (built by the CLI, handed in): for each parent target, in its stored order, the rows `spec tree <target> --depth 2` serves, in its order, the target first; each node once, at its first place. A row's `target` is its canonical REF as `resolved_list` stores a card's target (its ID, else its document's path); an ID-less section is passed over. A row with no kind holds `None`.
2. **Matched rows**: those whose `kind` equals a scope node's `kind`, byte for byte, in config order. `None` matches none.
3. **`suggested`**: the matched rows' disciplines, row order then list order, each once.
4. **Children**: `chosen` in written order; `None` (a dry run with no `--child`): every discipline the flows list, flow order then list order, each once. `chosen` empty: `Err("child: none named; name 1 to 32")`; over 32: `Err("child: <n> named; a split makes 1 to 32")`; a repeat: `` Err("child: `<d>` is named twice") ``; a name off `name_problem`: `Err("child: <problem>")`.
5. **Each child**: `flow_for(Some(d))` (an `Err` prefixed `child: `, the first in child order); `flow_title` the flow's `title`, else its name; `column` its first. Its kinds: the `kind`s of the matched rows listing `d`. Targets: the scope nodes of those kinds, scope order, `from` `kinds`; past 64, the first 64 and the note `` subtask `<d>`: <n> nodes of its kinds; the first 64 taken ``; none, or no matched row lists `d`: the parent's targets, `from` `parent`. `suggested`: `d` is in step 3's list.

`card-press`, `--target PRN-WEEKLY`: the scope holds `PRN-WEEKLY` and its `print-run` and `broadsheet` nodes under it (`PRN-PLATES`, `SHT-BACK`, `SHT-FRONT`, `SHT-INNER` and their ID'd sections); both rows match; `suggested` `copy`, `plates`; `copy` targets the `broadsheet` nodes, `plates` the `print-run` and `broadsheet` nodes, both `kinds`. `--target STY-0001` (a `style-ruling`, no row): `suggested` empty, every child `[STY-0001]`, `parent`.

### `spec card split`

```text
spec card split --target REF... --title T [--goal T] [--discipline D] [--child D]...
  [--reviewer N] [--as N] [--dry-run]
spec card split --parent C [--child D]... [--reviewer N] [--as N] [--dry-run]
```

Help: `--child D` `A subtask's discipline: one subtask per --child, first column of the flow listing it.`; `--dry-run` `Plan only: print what the split would make and write nothing.`; `--parent C` `Split under this stored card: only subtasks are made.`

Order of checks, each stopping:
1. **Usage** (exit 2): `--parent` with any of `--target`, `--title`, `--goal`, `--discipline`; no `--parent` and no `--title`; no `--child` without `--dry-run` (`` name each subtask's discipline with --child, or plan with --dry-run ``); `--as` off its grammar; a look-alike ID in `--parent` (its Latin form named); `C` in `[ids]` (`card_prefix_clash`).
2. **Config** (exit 2): a bad `[card]`.
3. **Fields** (exit 1): `field_problem` for `title`, `goal`, the target count; `discipline: <problem>`.
4. **Parent** (exit 1): `--parent C` read; none: `parent: no card C-0099`. Any state is taken, as `new --parent` takes it.
5. **Reviewer** (exit 1, a write only): D3's chain, then `reviewer: <problem>` with today's texts (`card-flows.md` "Hand-off").
6. **Targets** (exit 1): `--target` through `resolved_targets`, as `new`; the scope read.
7. **Plan** (exit 1): `split_plan`, the children's checks first ("The plan" step 4); a new parent's `flow_for(--discipline)`, its `Err` as `new` gives it.
8. **Store** (exit 1): `split_cards`; its `CardRefused` reason. A dry run stops before it.

Exits: 0 written or planned; 1 refused, nothing written; 2 cannot run, as `cards.md` "Commands" lists, plus the usage above.

### The split document

`--json`, `split_card`'s `structuredContent`, the route's 200 and 409; every key present, in this order:

```json
{"dry_run": false,
 "parent": {"id": "C-0001", "created": true, "title": "The weekly run", "discipline": "copy",
   "flow": "copy", "flow_title": "Copy desk", "column": "draft", "targets": ["PRN-WEEKLY"]},
 "children": [
   {"id": "C-0002", "discipline": "copy", "flow": "copy", "flow_title": "Copy desk",
    "column": "draft", "targets": ["SHT-BACK", "SHT-FRONT", "SHT-INNER"], "from": "kinds", "suggested": true},
   {"id": "C-0003", "discipline": "plates", "flow": "plates", "flow_title": "plates",
    "column": "queued", "targets": ["PRN-WEEKLY", "PRN-PLATES", "SHT-BACK"], "from": "kinds", "suggested": true}],
 "reviewer": "Ana Ruiz",
 "notes": []}
```

(Target lists abridged.) A stored parent: `created` false, its stored title, discipline, flow, column and targets. A dry run: every `id` null, `reviewer` the chain's result, unjudged, `null` when it ends at no name. A refusal (exit 1, a 409): `parent` null, `children` `[]`, `reviewer` null, its reason the last of `notes`. `notes`: the plan's, then the reason.

Text, one line each, `-` for none, through the one-line rule and the output cap:

```text
split C-0001 new: copy/draft | The weekly run | PRN-WEEKLY | reviewer Ana Ruiz
subtask C-0002: copy/draft | copy | SHT-BACK, SHT-FRONT, SHT-INNER | by kind, suggested
subtask C-0003: plates/queued | plates | PRN-WEEKLY, PRN-PLATES, SHT-BACK | by kind, suggested
note: <note>
```

A stored parent reads `split C-0001 stored: …`; `the task's targets` replaces `by kind`; a dry run prints `-` for each ID, `plan` for `split`, and ends `dry run: nothing written`. A refusal prints as a card's refusal does.

### The route: `POST .../cards/split`

`CardSplitBody`, `deny_unknown_fields`, every key optional at decode, the CLI's usage rules then applied:

```json
{"target": ["PRN-WEEKLY"], "title": "The weekly run", "goal": null, "discipline": "copy",
 "parent": null, "child": ["copy", "plates"], "reviewer": "Ana Ruiz", "dry_run": false, "as": "Ana Ruiz"}
```

`child` `null` or `[]` is none; `dry_run` absent or `null` is false. The door is `board-ui.md` "The door"'s (slug, same-origin, JSON, `CARD_BODY_MAX`, decode, `persons_named` for `as` and `reviewer`), then one `card_split` call in the project's turn at one clock reading. `split` is a static segment beside `.../cards/:id`: no card ID is `split`.

| Answer | When |
|---|---|
| 200 | the split document, written or planned |
| 400 | a body, a person or a usage rule refused, the CLI's line |
| 409 | refused (exit 1): the document, its reason the last of `notes` |
| 503 | cannot run: a bad `[card]`, a look-alike ID, `C` in `[ids]`, a corrupt row, a newer schema |

No 404: the path names no card, and a missing `parent` is a 409. Any other method: 405, `Allow: POST`. A written split adds one `card.created` per card it makes and nothing else; a dry run, none. **A hub**: the seventh plain write, `as` bound to the login (a sent other name is the plain class's 403, nothing stored), a dry run included.

### MCP: `split_card`

Input (`additionalProperties: false`): `target` (`["array", "null"]` of strings), `title`, `goal`, `discipline`, `parent`, `reviewer` (`["string", "null"]`), `child` (`["array", "null"]` of strings), `dry_run` (`["boolean", "null"]`); no `as`. Output: the split document. A refusal: an error result with the reason and the document; a call that cannot run: an error result with its line.

The stdio text, at most 2 048 ASCII bytes as `concat!` yields them, the tail `card_tail!()`:

```text
Splits work to departments in one act: the same as `spec card split (--target REF... --title T
[--goal T] [--discipline D] | --parent C) [--child D]... [--reviewer N] [--dry-run]`. It makes a
parent card, or takes the stored card parent, and one child card per child discipline, each in
the first column of the flow listing it, its parent the parent, its reviewer the split's. All
cards or none. content is the command's lines, structuredContent its --json document {dry_run,
parent, children, reviewer, notes}.

target: at most 64 nodes, resolved as a task's. title: required without parent. parent: a stored
card; then no target, title, goal or discipline. child: 1 to 32 disciplines, each once; none only
with dry_run. reviewer: a person, never agent or local; absent: the parent card's. dry_run: plan
only, nothing written; without child it plans every discipline. A child targets the nodes of the
kinds a [[card.split]] row names for it, found in the targets or two levels under them, else the
parent's targets; suggested marks a row's pick.
```

The hub's text: the same, "You act as your login" for the tail's first sentence, as the hub's three card writes have it; ASCII, at most 2 048, the determinism sentence, no tool left out named. The developer records both byte counts in "Implementation". `INSTRUCTIONS` and `HUB_INSTRUCTIONS` unchanged.

### Key sets and the plugin

`fixtures/daemon-keys.json`, 118 to 121 sets:
- 119 `CardSplit` `[dry_run, parent, children, reviewer, notes]`;
- 120 `CardSplitParent` `[id, created, title, discipline, flow, flow_title, column, targets]`;
- 121 `CardSplitChild` `[id, discipline, flow, flow_title, column, targets, from, suggested]`.

The body is a request, not a key set.

Plugin 0.37.0, MINOR (a tool added): `plugin.json`, the last `PINS` and `SCHEMA_PINS` entries, `scripts/deploy/.env.example`, `spec_ids_hub.rs`, `tag-check.sh`. `read-spec`'s body, 4 089 bytes of 4 096, one edit, net -3:

| Was | Becomes |
|---|---|
| ``queue rows beside the tasks, not in the spec: `list_cards` and `get_card` read them, `create_card`, `edit_card``` | ``queue rows, not in the spec: `list_cards` and `get_card` read them, `create_card`, `split_card`, `edit_card`…`` |

### The preset and the fixtures

`scripts/presets/gamedev.toml` gains, after `production`'s flow:

```toml
[[card.flow]]
name = "sound"
title = "Sound"
disciplines = ["sound"]
columns = ["backlog", "sent-back", "design", "recording", "implementation", "review", "in-game"]
titles = { backlog = "Backlog", sent-back = "Returned", design = "Sound design", recording = "Recording", implementation = "Implementation", review = "Review", in-game = "In game" }
accept = { from = "review", to = "in-game" }
return = "sent-back"
```

after the `art` workspace, before `production`:

```toml
[[workspace]]
name = "sound"
title = "Sound"
sections = ["board", "flow", "tree", "questions", "inbox"]
landing = "board"
flows = ["sound"]
```

and, after the commented direction:

```toml
# A split template: a task on a document of this kind, or holding nodes of
# it, pre-selects these disciplines in the Split dialog, and each subtask
# targets the nodes of its kinds. It never refuses a split. Use your project's
# own kinds, and add it only once every terminal, plugin and hub runs a build
# that reads `[[card.split]]`.
# [[card.split]]
# kind = "character"
# disciplines = ["narrative", "art", "sound"]
#
# [[card.split]]
# kind = "scene"
# disciplines = ["narrative", "level-design", "sound", "development"]
```

The header's version sentence becomes: `A flow's title and return, and a [[card.split]] row, need a build that reads them on every terminal, plugin and hub serving the project, an older one refusing the key.` (Comment text; the pins follow.) `fixtures/studio-lamp/specengine.toml` takes the new bytes between its marker lines. `fixtures/card-press/specengine.toml` gains "Data"'s two rows after its flows, and its header comment a sentence naming them; neither fixture's `expected.json` changes, since no read but the plan serves a row.

### The dialog

`SplitDialog`, a `Dialog` (`role="dialog"`, `aria-modal`, labelled by its heading), at most the work item's width (`ui-design.md` "The work item").

- **Openers.**
  - **The document** (`DocActs`, modal and page): "Split to departments…" after New task; the source is `{target: [REF], title: <the document's title, else its REF>}`.
  - **Flow's panel**: the scope (not an unrooted Top) and a box, after New task; the source is the node, as New task presets it.
  - **A task's work item** (`CardActs`): after Hand off, on an open card; the source is `{parent: C}`.

  Each opener holds `const SplitDialog = lazy(...)` and renders it under `Suspense` only once pressed; no opener preloads it.
- **The plan read**, on open, once: `splitCard` with `dry_run: true`, `child: null`, the source, a new parent's `discipline` prefill, `reviewer: null` and `as`. A skeleton while it reads; a refusal or failure: the daemon's words verbatim under `lang="en"`, and Retry, which reads again with the form's discipline. A change to the form reads nothing again.
- **The task part**, a new parent only: Title (prefilled, required), Goal (optional), Discipline (a select of every discipline the held workspaces read serves, each once, served order; prefilled with the chosen department's first `disciplines` entry, else empty).
- **Reviewer**: Hand off's field, `handOffReviewer(<the parent's reviewer, or null>, acting)`, editable and required locally, the read-only login on a hub (`card-flows.md` "Hand-off").
- **Subtasks**, a `fieldset` with a legend: one checkbox per plan child, checked when `suggested`. Rows group under the first department, in served order, whose `flows` names the row's `flow`; rows whose flow no department names come last under no heading. A row's label is its `flow_title`; under it, the raw `discipline` (`lang="en"`) when it differs from `flow`; then "By kind: <n> documents" (`from` `kinds`) or "The task's targets" (`from` `parent`), its targets in a collapsed `Disclosure` as `DocLink`s. A line "Suggested by the project's split templates" when any row is suggested.
- **Tasks on this document**, from a document only: the held trace read's cards for the REF, each a link to its work item; none, no line. No dedup is judged (W11).
- **Submit** "Split": off while no row is checked or a submit is out. It posts `dry_run: false`, `child` the checked rows' disciplines in row order, the form's title, goal, discipline, reviewer and the source. 200: the dialog closes, the opener's polite line "Split into C-0012 and 3 subtasks" (from a task: "3 subtasks for C-0012"), the ID a link to its work item. 409: the reason in the dialog's `role="alert"`, the fields kept. 400: likewise, one naming `--as` under the acting name as a card write's. 503: the daemon's words.
- Nothing computes a target, a suggestion or a placement: the page shows the served plan and sends the person's choice. No kind, flow, column or discipline is compared outside the grouping by served names.

### Catalog words

New keys in both catalogs, `board.split.*`; English values:
- `act` "Split to departments…"; `heading` "Split to departments", `headingCard` "Split {{id}} to departments";
- `title` "Title", `goal` "Goal", `discipline` "Discipline", `reviewer` "Reviewer";
- `subtasks` "Subtasks, one per department"; `suggested` "Suggested by the project's split templates"; `otherFlows` "Other flows";
- `byKind` "By kind: {{count}} documents" (plural forms); `fromTask` "The task's targets"; `noTargets` "No targets";
- `existing` "Tasks on this document:";
- `submit` "Split", `pending` "Splitting…", `none` "Choose at least one department";
- `done` "Split into {{id}} and {{count}} subtasks", `doneCard` "{{count}} subtasks for {{id}}" (plural forms).

Retry and the skeleton reuse `ui.*`. No value names a trade. Russian is the ui-developer's, in `ru.ts` alone, the owner's review open.

### Mocks

`src/mocks/cards.ts`, `MockClient`, harbor-sim: `POST .../cards/split` answers a dry run from a canned plan for each mock document, two of its rows `suggested`, one `from` `parent`; a write appends the parent and children to the mock's list, the parent's `revision` untouched when stored. `?scenario=conflict`: the first write answers 409, `child: no flow takes the discipline `x`: …`, nothing stored.

## Rules and edge cases

- WHEN any card of a split is refused, the store SHALL roll back the whole split: no row, no event, and the next card SHALL take the ID the refused split's parent would have taken.
- WHEN a split names a stored parent, its row, `revision`, `updated_at` and history SHALL stay byte-equal, and no event SHALL name it.
- WHEN a `[[card.split]]` row's kind matches no scope node, it SHALL suggest nothing and SHALL refuse nothing.
- WHEN a chosen discipline is in no matched row, its child SHALL target the parent's targets.
- WHEN the parent has no targets, the scope SHALL be empty, nothing suggested, and every child a manual card.
- WHEN a child's discipline is the parent's, it SHALL be a child like any other.
- WHEN a dry run is asked, nothing SHALL be written, and the persons SHALL NOT be judged; any other refusal SHALL be the write's.
- WHEN a child would target more than 64 nodes, it SHALL take the first 64 in scope order with a note, never a refusal.
- WHEN the same document is split twice, two sets SHALL be made: no dedup; the dialog lists the document's tasks.
- WHEN the parent, a child or the split is later moved, accepted or cancelled, no other card SHALL change (ADR-0074); a parent with open children SHALL be accepted as any card (ADR-0012).
- **Two stores.** A split names cards of one queue: a terminal's `--parent C-0007` is the terminal's card, never a hub's.
- **No core logic in the UI**: it shows the served plan and sends the checked disciplines; the daemon judges.
- Nothing branches on a kind, discipline, flow, column or department (ADR-0008); no trade's word enters the chrome.
- **Scale.** The scope is one subset tree read per parent target (at most 64), each to depth 2; the plan is linear in scope nodes times rows. A split writes at most 33 rows in one transaction. Unmeasured beyond R1.

### Rollout

**No storage step.** The queue stays at schema 11 and backup format 5; a split's cards dump and import as any.

**Older binaries.** `RawFile` only name-checks `card`, so an older `spec`, daemon or MCP server reading a `[card]` with `split` fails the card surfaces alone, `` unknown field `split`, expected `flow` ``: `spec card` exits 2, the card routes and the Board answer 503, Hand off loses its department selects (the workspaces read's `disciplines` empty, one note), the card tools fail. `spec show`, `check`, the gate and every other read are unaffected. Pinned on this build by an unknown `[card]` key.

**The rule** (ADR-0073's): commit a `[[card.split]]` row only after every terminal, plugin and hub serving the project runs a build that reads it. The preset's rows stay commented.

**The hub** runs `v0.36.0` at queue schema 11 and serves the departments' config. It upgrades once, after RM-0104 `task-mandate`, whose schema 12 needs a dump on the old build first (`hub-ops.md` "Upgrade"); this slice alone changes no stored byte there.

### Test churn

Only these change; every other test passes unchanged. The test-engineers add what is missing under "Implementation".

| Test | Becomes | Why |
|---|---|---|
| every test pinning `[card]`'s unknown-key message `` expected `flow` `` (core `card_toml` tests, CLI `cards.rs`, MCP `mcp_cards.rs`) | `` expected `flow` or `split` `` | D4 |
| tests pinning the preset's bytes, five flows or four workspaces, and `studio-lamp`'s copy: core `workspace_toml.rs`, `departments.rs`; CLI `departments.rs` (and `departments_before.json` where it holds `studio-lamp`'s flows), `workspace_cli.rs`; http `workspaces.rs`; eval `workspace_names.rs`, `anonymity.rs`; `fixtures/studio-lamp/expected.json` | six flows, five workspaces, the `sound` lines | D11 |
| goldens of `studio-lamp`'s `spec card list` text or `flows[]` (CLI `cards.rs`, `card_handoff.rs`, `card_questions.rs`, `card_tasks.rs`, `store_reads.rs`; http `cards.rs`, `card_questions.rs`, `hub_team.rs`, `query.rs`; store `queue_cards.rs`) | a `flow sound` line, a sixth `flows[]` entry | D11 |
| MCP tool counts: `mcp_cards.rs`, `mcp_review_form.rs`, `mcp_trace.rs`, `plugin_skills.rs` (29), `mcp_workspaces.rs`' text; the hub's (`mcp_hub.rs` and `HUB_TOOLS`' pins, 17) | 30; 18 | D8 |
| http `door.rs` (`CARD_CALLS` 12, the third exception), `hub.rs` unit tests of `class` and the route count (28), `methods.rs`, `daemon_keys.rs`, `no_domain.rs`; `fixtures/daemon-keys.json`; UI `api/daemonKeys.test.ts` | the route, 29 routes, 121 sets, the rows' disciplines as words | D7, D11 |
| `plugin_files.rs`, `plugin_skills.rs` (read-spec's sentence), http `spec_ids_hub.rs`, `tag-check.sh` | 0.37.0 | D14 |
| UI `board/CardActs` tests, the document view's acts tests, Flow's panel tests, `mocks/MockClient.test.ts` | the new act after New task or Hand off | D10 |

## Acceptance criteria

A browser look is marked **[look]**; a timed or counted run, **[measure]**. Each is recorded in "Implementation" with its date. Every other criterion is a test that the named mutation turns red. New Rust tests live in `card_split.rs` of the crate named (core, store, CLI, http, mcp), unless another file is named. Scratch copies of `fixtures/card-press` (two rows, no game word) and `fixtures/studio-lamp` (the preset) serve them.

- [ ] S1 — **Config.** Core:
  - `card-press`'s `[card]` reads `splits` `[{broadsheet, [copy, plates]}, {print-run, [plates]}]`, config order; a `[card]` without the key reads `splits` empty and every flow as before;
  - each line of "Data"'s error table, at its line, from a probe config each; a row naming `voice`, which no flow lists, is the listing error;
  - the preset with its two rows uncommented reads, six flows and two rows;
  - CLI: `spec card list` on a copy whose row names `voice` exits 2 with that line; `spec show PRN-WEEKLY` and `spec check` on it exit as before.

  Mutations, each red: the listing check dropped (the `voice` row reads); a kind compared trimmed; `split` judged before every flow reads (a flow's error masked).
- [ ] S2 — **The plan.** Core, `split_plan` on built configs and scopes:
  - scope `[(A, scene), (A-VO, voice), (A-X, None), (B, voice)]`, rows `scene: [narrative, sound]`, `voice: [sound]`, chosen none: `suggested` `[narrative, sound]`; `sound` targets `[A, A-VO, B]`, `kinds`; `narrative` `[A]`, `kinds`; a third flow's discipline `art`, `[parent targets]`, `parent`, not suggested;
  - a row `Scene` matches no `scene` node; a `None` kind matches none;
  - 70 scope nodes of one kind: 64 targets and the note;
  - chosen `[art, sound]` keeps that order; `[]`, a 33rd, a repeat, `nowhere` (no flow) each `Err` with "Data"'s line, `nowhere`'s `flow_for`'s prefixed `child: `.

  Mutations, each red: kinds matched case-insensitively; `suggested` in discipline order instead of row order; a child with no kind node given `[]`; the cap at 65.
- [ ] S3 — **A split from a document.** CLI, `card-press`:
  - `spec card split --target PRN-WEEKLY --title "The weekly run" --discipline copy --child copy --child plates --reviewer "Ana Ruiz" --as "Ana Ruiz" --json`: exit 0; `C-0001` `copy/draft`, targets `[PRN-WEEKLY]`, reviewer Ana Ruiz, no assignee; `C-0002` `copy/draft` and `C-0003` `plates/queued`, each `parent` `C-0001`, reviewer Ana Ruiz, title and goal the parent's, targets as "Data", the plan's lists pinned from the first green run (`SHT-BACK`, `SHT-FRONT` and `SHT-INNER` in `copy`'s, no `PRN-` there);
  - `spec card list --parent C-0001` lists `C-0002`, `C-0003`; each `show` holds a snapshot per target;
  - the event log gains exactly three `card.created`, `C-0001` first, each payload `new`'s keys, `as` Ana Ruiz;
  - the text output is "Data"'s three lines.

  Mutations, each red: children inserted before the parent; a child's reviewer left null; a child's `goal` dropped.
- [ ] S4 — **A split under a stored task.**
  - `spec card new --title T --discipline copy --reviewer "Ana Ruiz"` makes `C-0001`; `spec card split --parent C-0001 --child plates --json`: one child, `parent` `C-0001`, reviewer Ana Ruiz; `C-0001`'s row, `revision` and `updated_at` byte-equal; no event names it; `parent.created` false;
  - `--parent C-0001 --title X`: exit 2; `--parent C-0099`: exit 1, `parent: no card C-0099`, nothing written, the next `new` takes `C-0002`; a Cyrillic look-alike: exit 2 naming `C-0001`;
  - a stored parent with no reviewer, no `--reviewer`, no `--as`: exit 1, `` reviewer: `local` can be no reviewer: a person accepts a result ``.

  Mutations, each red: the parent's `revision` raised; the chain skipping the stored parent's reviewer.
- [ ] S5 — **All or none.**
  - Store: `split_cards` with a new parent and two children, the second's reviewer `agent`: `CardRefused`, `reviewer: …`; no card row, no event; `create_card` next takes `C-0001`;
  - store: `Stored("C-0099")`: `CardRefused`, nothing written;
  - CLI: `--child copy --child nowhere`: exit 1, `child: no flow takes the discipline `nowhere`: …`, nothing written; `--child copy --child copy`: exit 1, `` child: `copy` is named twice ``.

  Mutations, each red: each card committed alone (the first stays); the children's `parent` taken from the input rather than set in the transaction.
- [ ] S6 — **Dry run.**
  - `spec card split --target PRN-WEEKLY --title T --discipline copy --dry-run --json`: exit 0, `dry_run` true, every `id` null, `children` `copy` then `plates`, both `suggested`; no card row, no event, the last event's `seq` unchanged;
  - `--target STY-0001 --title T --discipline copy --dry-run`: both `suggested` false, `from` `parent`, targets `[STY-0001]`;
  - `--reviewer agent --dry-run` with children: exit 0; the same without `--dry-run`: exit 1;
  - `--dry-run` with `--child nowhere`: exit 1, the write's line; a bad `[card]`: exit 2.

  Mutations, each red: a dry run storing; a dry run judging the reviewer.
- [ ] S7 — **The route and the hub.** http:
  - local: S3's split as a body, 200 with the document; a dry run 200, nothing stored; `child` naming `nowhere` 409 with the reason last; an unknown key or a bad `as` 400; a bad `[card]` 503; GET 405 `Allow: POST`; `GET .../cards/C-0001` still answers the card;
  - `door.rs`: `CARD_CALLS` 12 with `card_split`; `card_writes_touch_only_the_queue` meets a split adding three `card.created` and nothing else, a dry run none;
  - a hub: a login outside `[team]` splits (200), `as` absent records the login on every `card.created`, `as` naming another 403 with nothing stored; `class` gives `Plain` for POST `.../cards/split` and `None` for DELETE.

  Mutations, each red: the arm left out of `class` (403); `acting` not called (the event's `as` `local`).
- [ ] S8 — **MCP.** mcp:
  - stdio `tools/list` holds 30 with `split_card`, `additionalProperties: false`; a call with no `reviewer` on a new parent: an error result, `` reviewer: `agent` can be no reviewer… ``, nothing stored; with `reviewer` a person: stored, `as` `agent`; `dry_run` true: the plan, nothing stored; an unknown key: refused;
  - a hub's `/mcp` lists 18; its `split_card` records the login; both texts ASCII, at most 2 048 bytes, holding the determinism sentence, naming no tool left out.

  Mutations, each red: `split_card` missing from `HUB_TOOLS`; the stdio tool acting as `local`.
- [ ] S9 — **The preset, fixtures and guards.**
  - the preset holds "Data"'s `sound` flow, `sound` workspace (before `production`), header sentence and commented rows verbatim; `studio-lamp`'s copy is byte-equal; `spec workspace --json` on `studio-lamp` lists five workspaces, `sound` serving `disciplines` `[sound]`;
  - `no_domain.rs` takes every `[[card.split]]` row's `disciplines` as words (its kinds already come with `kind = "…"`); a planted `"plates"` in a probe source is red; `workspace_names.rs` finds no `sound` literal; `anonymity.rs` passes;
  - an unknown `[card]` key fails `spec card list` alone (exit 2), `spec show` and `spec check` as before: the older-binary pin.

  Mutations, each red: the header sentence dropped; `no_domain.rs` not reading the rows' disciplines.
- [ ] K1 — **No cascade** (ADR-0074, ADR-0012). CLI, after S3's split: moving, accepting and cancelling `C-0002` changes no other card's row; cancelling `C-0001` leaves both children open; `C-0001` moves to `proof` and is accepted with both children open; accepting both children moves no parent.

  Mutation, red: a split raising the stored parent's `revision`, or an accept refusing a parent with open children.
- [ ] P1 — **Plugin 0.37.0.**
  - `plugin_files.rs`: `PINS` and `SCHEMA_PINS` end at 0.37.0, the schemas' hash moved; `spec_ids_hub.rs` and `tag-check.sh` read 0.37.0;
  - `plugin_skills.rs`: `read-spec`'s body at most 4 096 bytes, naming `split_card`, every content rule passing.

  Mutations, each red: `plugin.json` left at 0.36.0; `split_card` dropped from `read-spec`.
- [ ] U1 — **Openers and the chunk.** UI:
  - `DocActs` shows "Split to departments…" after New task, modal and page; Flow's panel after New task for the scope (none at an unrooted Top) and a box; `CardActs` after Hand off on an open card, none on an accepted or cancelled one;
  - pressing opens the dialog with the source ("Data");
  - after `pnpm build`, `SplitDialog` is its own chunk, absent from first load and from every `modulepreload`.

  Mutation, red: a static `import { SplitDialog }` in `CardActs.tsx`.
- [ ] U2 — **The dialog.** `src/board/SplitDialog.test.tsx` (new), on the mock:
  - one plan read on open, `dry_run: true`, `child: null`; rows in served order under their departments, `suggested` ones checked, the `parent` row reading "The task's targets", the suggested line shown;
  - from a document: Title prefilled, Discipline the chosen department's first, the document's tasks listed; from a task: no Title, Goal or Discipline;
  - Submit off with none checked; toggling reads nothing; Split posts `dry_run: false` and `child` the checked disciplines in row order; 200 closes with the polite line and link; the conflict scenario's 409 keeps the fields with the reason in the alert;
  - a hub login: Reviewer read-only, the login.

  Mutations, each red: Split sending every suggested row whatever is checked; a plan read per toggle.
- [ ] U3 — **[look]** Chrome DevTools, light and dark, English and Russian, at 1440x900 and 320 wide, on the mock and on a scratch copy of a real game corpus with the preset's flows and two `[[card.split]]` rows of its own kinds: the act at each opener; the dialog with suggested rows, a `parent` row and the targets open; a refusal; the polite line; the parent's Subtasks after a split; long Russian labels wrapping; at 320 the dialog filling the screen; focus rings visible, nothing cut unseen; the Network panel's list reads after one split recorded.
- [ ] U4 — **[measure] Size.** After `pnpm build`, first-load JS (`ui-languages.md` "Size") is at most 845 kB; expected +1 to +3 kB on the figure step 6b ships with (815.11 kB at step 6), the dialog lazy, the acts and English words in first load. Record the figure, gzip, `SplitDialog`'s chunk and `ru-*.js`.
- [ ] R1 — **[measure] A real corpus.** On a scratch copy of a real game corpus with two rows of its kinds, a dry run through the route on five of its largest documents: record each one's scope size, `suggested`, each child's target count and any cap note, and the call's time; a time over 500 ms is a finding for the review.
- [ ] G1 — **Gates.**
  - **UI.** Lint 0, `tsc` clean, the full test run green twice.
  - **Rust.** The workspace run green, then `cargo nextest run -p specengine-eval --test doc_pointers --test anonymity --test ui_policy --test workspace_names`.
  - **Docs.** The check is clean.

## Out of scope

- The mandate, binding or the department's to decide: RM-0104 `task-mandate`, the next slice.
- The old item's soft `after` and any order or wait between subtasks (ADR-0074, ADR-0012).
- Grandchildren in one act: a split makes one level, and a row never nests.
- A template naming a department or matched by anything but a kind; editing rows from the page.
- Editing a child's targets, title or goal inside the dialog (each is edited after, on its task).
- Dedup of splits; notifications; department Overview counts (RM-0086).
- A split across two stores; a split from a hub's card into a terminal's.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-core/src/{card_toml,card_split,lib}.rs`; `crates/specengine-store/src/queue/cards.rs`; `crates/specengine-cli/src/{card,main}.rs`; `crates/specengine-mcp/src/{cards,hub,mirror,server}.rs`; `crates/specengine-http/src/{app,hub}.rs`; `plugin/specengine/.claude-plugin/plugin.json`, `plugin/specengine/skills/read-spec/SKILL.md`; `scripts/presets/gamedev.toml`, `scripts/deploy/.env.example` |
| test-engineer, Rust | `card_split.rs` in core, store, CLI, http and mcp (new); "Test churn"'s Rust rows; `fixtures/card-press/specengine.toml`, `fixtures/studio-lamp/specengine.toml` and its `expected.json`, `fixtures/daemon-keys.json` |
| ui-developer | `src/api/provisional.ts`, `client.ts`, `http.ts`, `queries.ts`; `src/board/SplitDialog.tsx` (new, lazy), `CardActs.tsx`; `src/doc/DocBody.tsx`; `src/flow/FlowView.tsx`; `src/i18n/{en,ru}.ts`; `src/mocks/cards.ts`, `MockClient`; the tests of U1-U2 |
| test-engineer, UI | gates, mutations, U4; the UI rows of "Test churn"; `ui_policy.rs` and `doc_pointers` if a pin moves |

Comments cite `docs/features/department-split.md` "Acceptance criteria" with a criterion ID, or an existing canon heading; never "Data" or "Description and interactions", which compaction drops at ship (doc_pointers).

### Work order

1. Step 6b `room-and-scroll` ships first: the UI lane is busy with it, and U4's figure is taken on its build.
2. The rust-developer and the ui-developer then start together; the contract is fixed above. The UI runs on the mock, the three key sets in `AHEAD_OF_FIXTURE` until the Rust lane's `daemon-keys.json` lands.
3. The Rust test-engineer overlaps the Rust lane: S1-S2 first (core, pure), then S3-S9, K1, P1, with the fixture rows and the churn. Then the Rust review.
4. The UI lane completes; the UI test-engineer runs the gates and mutations (U1, U2, U4); then the UI review.
5. The orchestrator's look (U3) and R1, on the corpus copy.
6. Ship: the spec-writer ("Canon at ship").

At most three iterations a lane.

**The cut line**, if the UI lane runs out:
- **Moves to a new plan item**, `department-split-ui`, the next free `RM-` number: the dialog and its three openers. The ship's spec-writer creates it, `must` in `v5`, before `task-mandate`'s UI.
- **Ships**: the Rust part whole (`[[card.split]]`, the plan, `spec card split` and `--dry-run`, the route, the hub's plain write, `split_card`, the preset, plugin 0.37.0).
- **The UI lane still lands the types**, `daemonKeys.test.ts` with `AHEAD_OF_FIXTURE` empty.

### Risks

- **Older binaries** (D15): a `[card]` with a row stops an older build's Board, Hand off's departments and card tools; the preset keeps its rows commented, and the hub upgrades once after `task-mandate`.
- **A typo in a row** stops every card surface of this build too (S1); `spec card list` names its line, and `spec check` does not.
- **Size**: 29.89 kB of headroom at step 6; the dialog lazy, the words the only first-load growth (U4).
- **The genre and anonymity**: rows' kinds and disciplines are a project's words; `no_domain.rs`, `workspace_names.rs`, `anonymity.rs` guard them; the preset's kinds are generic, never a pilot's.
- **Catalogs**: every key in both, plural forms for counts.
- **Churn**: `studio-lamp` holds the preset, so its list goldens gain a sixth flow; the tool counts; the key sets; the plugin pins; `read-spec` 7 bytes under its cap.
- **Duplicates**: no dedup; the dialog lists the document's tasks.
- **Event bursts**: one split logs up to 33 `card.created` in a transaction; the page's event stream re-reads per event and the query cache folds in-flight reads; U3 records the list reads.
- **Hub accepts**: a subtask's reviewer accepts on a hub only as a `[team]` login (`card-flows.md` "Known limits").
- **Caps** at ship: below.

### Canon at ship

`docs/canon/card-split.md` loses "State" and gains the rest, at most 12 KB: Config, the plan, the command and its document, the route and the hub, MCP, the screen, Known limits (the rollout, a typo's reach, no dedup, two stores), Tests.

Each edit below stays within its cap; free bytes now in brackets. "Net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/cards.md` [114] | "Commands": a `spec card split` pointer to `card-split.md`; "MCP": six tools with `split_card` (net not positive) |
| `docs/canon/card-flows.md` [169] | "Config": `[card]` holds `flow` and `split` (pointer); "Known limits": older binaries refuse `split` too (net not positive) |
| `docs/canon/workspaces.md` [35] | "Preset": six flows, five workspaces with `sound`, the commented rows (net not positive) |
| `docs/canon/board-ui.md` [5 352] | "Routes": the row; "Methods"; "Fence and trust": seven plain writes; `CARD_CALLS` 12, a split's events the third exception |
| `docs/canon/board-screen.md` [153] | "Writes and dialogs": Split to departments… (pointer) |
| `docs/canon/doc-view.md` [2 059] | "The view", Head's acts: Split to departments… after New task |
| `docs/canon/spec-flow-screen.md` [83] | the scope's and a box's acts (net not positive) |
| `docs/canon/hub.md` [13], `docs/canon/hub-team.md` [55] | seven plain writes, `.../cards/split` (net not positive) |
| `docs/canon/hub-mcp.md` [7] | eighteen tools, `split_card` (net not positive) |
| `docs/canon/serve.md` [8] | "Trust": a split under any name locally (net not positive) |
| `docs/canon/proposal-review.md` | key sets 121 |
| `docs/canon/plugin-files.md` [4 975], `docs/canon/plugin.md` [322] | 0.37.0; `read-spec`'s tools |
| `docs/canon/hub-deploy.md` [2] | the version it names (net not positive) |
| `docs/canon/ui-languages.md` [503] | "Size": the figure |
| `crates/specengine-{core,store,cli,mcp,http}/README.md` [9, 3, 10, 10, 1] | only where they name the keys, calls, routes or tool counts (net not positive) |
| `CLAUDE.md`; 08 s2 Phase 7 step 6c; RM-0100 | shipped |
| this spec | compacted to at most 3 KB |

`docs/index-canon-card.md` gains `card-split.md`'s line at spec time already; `card-flows.md` "The rule" changed with ADR-0078 at spec time.

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner did not answer:
- Templates are `[[card.split]]` rows under `[card]` (D4), never a top-level key, so an older build fails its card surfaces, not the whole file.
- A child targets its rows' kinds' nodes, else the document, the parent's targets (D5).
- The mandate is the next slice, RM-0104, not this one.
- Two slices, then one hub upgrade after both.

**The spec-writer's:**
- W1: the route is `POST .../cards/split`, a static segment beside `:id`; library `card_split`, handler `split_card`; a missing parent is a 409, since the path names no card.
- W2: the reviewer's chain is given, then the stored parent's, then the acting name, judged as `assign` judges (D3).
- W3: a dry run judges every rule but the persons; with no `--child`, it plans every discipline the flows list, so the dialog reads once.
- W4: a child takes the parent's title and goal; nothing else is copied.
- W5: the scope is each parent target and its tree rows to depth 2, the read a document's Inside makes; an ID-less section is passed over.
- W6: a child over 64 nodes takes the first 64 with a note, never a refusal.
- W7: a row's discipline no flow lists is a config error, which catches a typo where the project writes it; a kind has one row.
- W8: a stored parent in any state is taken, as `new --parent` takes one; the page offers the act on open tasks only.
- W9: `card.created` is unchanged; only `parent` tells a split's cards.
- W10: the plan serves `flow_title`, so the dialog needs no cards read; rows group under the first department whose `flows` names their flow, so an all-flows department does not swallow them.
- W11: no dedup; the dialog lists the document's tasks.
- W12: at most 32 children and 64 rows.

**The owner's questions:**
- **Q-A.** Should a row name departments rather than disciplines? Working answer: disciplines, the flows' own words; the dialog shows departments.
- **Q-B.** Should subtasks wait on each other, as the old soft `after` would? Working answer: no (ADR-0074, ADR-0012); a later item if the trial asks for one.
- **Q-C.** Should a subtask's title carry its department? Working answer: the parent's title; the department's board tells them apart, and each is edited on its task.
