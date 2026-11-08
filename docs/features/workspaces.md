---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, ui, scripts, fixtures]
ref: RM-0081; v4 step 1a of the owner's re-scope of 2026-11-08 (08 s2 Phase 7, "v4 scope"), his decisions 1 and 3; Q5 and Q6 are the orchestrator's working answers, awaiting the owner's review
adrs: [ADR-0073]
---

# workspaces: a workspace per trade, modules, a gamedev preset, and Tasks off the nav (RM-0081)

## Why

### Requirement

Every person sees the engine's nine sections, whatever their trade (`ui/src/app/sections.ts`). The owner asked on 2026-11-08 for scopes per role: a writer needs some pages and a developer others, and a writer has no use for Health. He also asked to split a project into modules, so a person sees what belongs to one part of the game. His decisions: (1) four roles to start with, writer, developer, artist or level designer, and lead; (3) Tasks leaves the nav, while the `T-` model and the gate stay. He accepted the outline: workspaces per role from project config, a gamedev preset outside the core (ADR-0008), a local switcher, and modules as the top navigation.

This step gives the shell a lens per trade and gives Tree and Flow a module bar. Every later v4 slice reads the workspace: the studio (2b), the board (3), the dashboards (4).

### Decisions

- **D1 A lens (ADR-0073).** A workspace holds the sections its nav lists, in its order, a landing section, the card flows its Board shows, and a default module. It never hides a route, entitles anyone, or stops anything (ADR-0012). Each section keeps its hash route and its palette entry.
- **D2 Two tables, read apart.** `[[workspace]]` and `[[module]]` in `specengine.toml`. A new core reader, `WorkspaceConfig::from_toml`, of `card_toml`'s family, reads them; `ProjectConfig` only name-checks them in `RawFile`. The reader never fails. A malformed table is replaced by its default with one note; a wrong reference inside a good table is left out with a note. No read, command, check or gate fails on either.
- **D3 The implicit workspace.** With no `[[workspace]]`, or a malformed one, the served document holds one workspace, `all`: the nine sections minus `tasks`, in `sections.ts` order, landing on the Overview, every card flow and no module. That is decision 3 for every project that configures nothing.
- **D4 A module is one root (Q5, narrowed).** `root` is a REF in any `spec show` form, and the module is that document and its subtree: the `root` the Tree and Flow reads already take. Q5's kind selector, a module for each document a selector matches, is left out. It would turn the route from a config read into a corpus read, taking a hub's turn on every page load, and neither read takes more than one root. The grammar leaves room: today a selector key is an unknown key, so its table falls back with a note.
- **D5 Served by the daemon, with a CLI twin.** The route is `GET /api/projects/:p/workspaces`, with no query and no turn: it reads the config alone, as `/api/projects` does. Its document is `spec workspace --json` byte for byte. `spec workspace` is read-only, like `spec team`. No MCP tool is added: an agent works on documents and cards, not on a viewer's nav.
- **D6 "Workspace", not "View as" (risk 10).** "View as" reads as acting as someone else. On a hub it would sit beside "Acting as", the login (`docs/canon/hub.md` "Screen"). The control is a select named **Workspace**, its hint saying it changes no right. It shows only when two or more workspaces are served.
- **D7 Tasks off the nav, by data.** The implicit list and the preset leave `tasks` out. The grammar still takes it: the section IDs are the engine's closed vocabulary, and a project may keep it in a workspace. Its route, its palette entry and the Inbox's links stay (ui-live-tasks AC-12). A task shown inside its card is step 3's.
- **D8 The choice is per viewer and per project.** It lives in `localStorage` `specengine.workspace.<p>`, as `specengine.board.as` does (`docs/canon/board-ui.md` "The acting name"). With no stored choice, the first workspace applies. A hub login's default is RM-0089's, and `[team]` is untouched (Q6).
- **D9 The landing.** `#/` opens the first project's landing in its chosen workspace, and choosing a workspace opens its landing. A typed `#/<p>` stays the Overview, and no route is ever redirected.
- **D10 The module bar.** It shows on Tree and Flow only, the two reads it roots, and only when `[[module]]` serves one. With no module table there is no bar: none is made up from the tree.
- **D11 The preset lives at `scripts/presets/gamedev.toml`.**
  - It is outside every crate (ADR-0008). `plugin/` is a closed, pinned set of files (`docs/canon/plugin-files.md` "Changing it"). `fixtures/` holds test corpora, not files a project copies.
  - `scripts/` is the rust-developer's area. It already holds setup files a person copies or runs (`scripts/deploy/`'s compose, Authelia and Traefik files), so no `.claude/` edit is needed before the build.
  - RM-0061's `spec init --pack` may later embed it or move it.
  - A top-level `presets/` was weighed and set aside: no role writes there, so the owner would first have to edit `.claude/agents/`.
- **D12 1a's strings stay in one module.** Every new chrome string is a member of `WORKSPACE_TEXT` in `ui/src/workspace/text.ts`. A string around data is a function of that data, never words joined in a component. So 1b (`ui-languages`, RM-0082) turns the module into catalog keys without touching a component. Section labels stay in `sections.ts`, which 1b converts too.

### Assumptions

- **A1** The role research changes only the preset's data, never code.
- **A2** Choosing a module scopes Tree and Flow; the Board's and the Roadmap's module filters wait for later slices.
- **A3** A module's root is resolved by the read it roots. A root naming nothing is that read's own exit-1 `reason`, shown as the view's error.
- **A4** The UI and the daemon ship in one binary, so a served section ID the UI does not know comes only from the mock. It is not drawn.

## Description and interactions

In scope:
- core `workspace_toml.rs`, and `RawFile`'s two keys;
- CLI `spec workspace [--json]`;
- the daemon's `GET .../workspaces`, local and on a hub;
- the UI: the nav, the Workspace switcher, the landing, the config notes, the module bar on Tree and Flow, and the Board's flows;
- `scripts/presets/gamedev.toml`;
- the guards, the fixtures.

What a person sees:
- The nav reads Overview, then the chosen workspace's sections.
- A project with two or more workspaces shows the Workspace select in the shell's header. Choosing one opens its landing.
- On Tree and Flow, a project with modules shows a bar, All modules and then each module's title. Choosing one roots both views there.
- The Board shows the workspace's card flows. When flows are hidden, a line counts them and a button shows them all.
- A config problem shows as "Config notes (n)" beside the switcher: never an error page and never a blank nav.

An agent sees nothing new. `spec workspace` is there for the person who writes the config, and for tests.

Interactions:
- `[card]` (`docs/canon/cards.md`): the reader judges `flows` against `card_from_toml`'s flow names. A card's flow and discipline do not change.
- `[team]` (`docs/canon/hub-team.md`) is never read by this route, nor `[team]` by this reader.
- The Tree and Flow reads are unchanged: the module only fills the `root` they already take (`docs/canon/spec-flow-screen.md` "Reads").
- The palette (`ui/src/palette/options.ts`) keeps every section: it calls `navOf` with all nine.
- The hub mounts the route for every login, as a plain read.

## Data

### Tables (`specengine.toml`)

| `[[workspace]]` key | Type | Rule | Absent |
|---|---|---|---|
| `name` | string, required | `card_toml::name_problem`'s grammar `[a-z0-9_-]`, 1 to 64 bytes; once among workspaces | -- |
| `title` | string | any script, 1 to 64 UTF-8 bytes, no control character | the name |
| `sections` | array of strings, at most 32 | each one of the nine section IDs: `inbox`, `tasks`, `board`, `roadmap`, `flow`, `tree`, `graph`, `health`, `questions` | the implicit eight |
| `landing` | string | `overview`, or one of its kept sections | `overview` |
| `flows` | array of strings, at most 32 | each a `[[card.flow]]` name | `null`: every flow |
| `module` | string | a served `[[module]]` name | `null` |

| `[[module]]` key | Type | Rule | Absent |
|---|---|---|---|
| `name` | string, required | the same grammar; once among modules | -- |
| `title` | string | as a workspace's | the name |
| `root` | string, required | 1 to 256 bytes, no control character, no space at either end; a REF in `spec show`'s forms, not resolved here | -- |

At most `MAX_WORKSPACES` (32) workspaces and `MAX_MODULES` (64) modules. Both keys hold arrays of tables; `workspace = []` or `module = []` is the same as no key.

The example below appends to `fixtures/card-press/specengine.toml`:

```toml
[[workspace]]
name = "newsroom"
title = "Newsroom"
sections = ["board", "tree", "inbox"]
landing = "board"
flows = ["copy"]
module = "weekly"

[[workspace]]
name = "pressroom"
title = "Pressroom"
sections = ["board", "flow", "tree", "health"]
flows = ["plates"]

[[module]]
name = "weekly"
title = "The weekly run"
root = "PRN-WEEKLY"

[[module]]
name = "rulings"
title = "Style rulings"
root = "STY-0001"
```

### The reader (`crates/specengine-core/src/workspace_toml.rs`)

- `WorkspaceConfig::from_toml(text: &str) -> WorkspaceConfig` (and `workspace_from_toml`) is pure and never fails. It returns `{source, workspaces, modules, notes}`.
  - `source` is `WorkspaceSource::{Config, Implicit}`.
  - `notes` is a `Vec<WorkspaceNote {line, message}>`, sorted by line, ties in judging order; `notes_at(file)` renders each as `<file>:<line>: <message>`.
- Constants:
  - `SECTIONS: [&str; 9]`, `sections.ts`'s IDs in its order;
  - `IMPLICIT_SECTIONS: [&str; 8]`, the same without `tasks`;
  - `OVERVIEW = "overview"`, `IMPLICIT_NAME = "all"`, `IMPLICIT_TITLE = "All sections"`;
  - `MAX_WORKSPACES`, `MAX_MODULES`, `MAX_LIST = 32`, `MAX_ROOT_BYTES = 256`.

  Names and titles reuse `card_toml`'s `MAX_NAME_BYTES` and `MAX_TITLE_BYTES` (64).
- **Judging order**:
  1. `[[module]]`. A malformed table is any of: not an array of tables, an unknown key (the TOML reader's message), a wrong type, a missing `name` or `root`, a bad name, title or root, a name twice, more than 64. It leaves no module, with one note: the first problem by line.
  2. `[[workspace]]`. Absent, the implicit workspace, with no note. A malformed table is any of the same, more than 32 workspaces, or more than 32 entries in a list. It serves the implicit workspace, with one note.
  3. Each workspace, in config order:
     - `sections`: an unknown or repeated ID is left out, with a note.
     - `landing`: anything but `overview` or a kept section becomes `overview`, with a note.
     - `flows`: judged against `card_from_toml(text)`. An unknown or repeated name is left out, with a note. None left, or none given, is `null`. If `[card]` cannot be read, the lists are served as written, with one note for the document.
     - `module`: a name no served module has becomes `null`, with a note.
- The line of a note is that of the offending value; for a missing key, the table header's; else 1.

### Notes, exactly

`<config>` is the config as given, as every config error writes it (`crates/specengine-cli/README.md` "Commands", its **Discovery**). Each note is one line:

```text
<config>:<line>: <problem>; the implicit workspace is served
<config>:<line>: <problem>; no module is served
<config>:<line>: workspace `<w>`: `<s>` is no section (inbox, tasks, board, roadmap, flow, tree, graph, health, questions): left out
<config>:<line>: workspace `<w>`: section `<s>` is listed twice: the second left out
<config>:<line>: workspace `<w>`: no `[[card.flow]]` is named `<f>`: left out
<config>:<line>: workspace `<w>`: flow `<f>` is listed twice: the second left out
<config>:<line>: <card's problem>; the workspaces' flows are served unjudged
<config>:<line>: workspace `<w>`: landing `<l>` is not one of its sections: it lands on the Overview
<config>:<line>: workspace `<w>`: no `[[module]]` is named `<m>`: no default module
```

`<problem>` is the reader's message: `name_problem`'s, the TOML reader's for an unknown key or a wrong type, or `` `[[workspace]]` is required to hold `name` `` for a missing key.

### `spec workspace [--json]` (`crates/specengine-cli/src/workspace.rs`)

It reads the config once (`discover_with_text`), takes no index and writes nothing. Output on card-press with the tables:

```text
workspaces config | workspaces 2, modules 2
workspace newsroom | Newsroom | landing board | sections board, tree, inbox | flows copy | module weekly
workspace pressroom | Pressroom | landing overview | sections board, flow, tree, health | flows plates | module (none)
module weekly | The weekly run | root PRN-WEEKLY
module rulings | Style rulings | root STY-0001
```

`flows (all)` stands for `null`. Parentheses are no name character, so neither form can be a name.

JSON: every key present, in this order.

```json
{"source":"config","notes":[],
 "workspaces":[
  {"name":"newsroom","title":"Newsroom","landing":"board","sections":["board","tree","inbox"],"flows":["copy"],"module":"weekly"},
  {"name":"pressroom","title":"Pressroom","landing":"overview","sections":["board","flow","tree","health"],"flows":["plates"],"module":null}],
 "modules":[
  {"name":"weekly","title":"The weekly run","root":"PRN-WEEKLY"},
  {"name":"rulings","title":"Style rulings","root":"STY-0001"}]}
```

The implicit document, on `fixtures/spec-a`:

```json
{"source":"implicit","notes":[],"workspaces":[{"name":"all","title":"All sections","landing":"overview","sections":["inbox","board","roadmap","flow","tree","graph","health","questions"],"flows":null,"module":null}],"modules":[]}
```

Streams and exits:
- Notes follow the CLI's rule: one `note:` line each on stderr, and JSON `notes` (`crates/specengine-cli/README.md` "Exit codes and streams").
- Exit 0 whatever the tables say.
- Exit 2 on usage, no project, or a config `ProjectConfig` refuses.
- No cut: the limits bound the document, as they bound `spec team`'s.

### The daemon

- **Route.** `GET /api/projects/:p/workspaces`, through `answer.rs`'s one CLI call. The answer is 200 and the document above.
- **No turn.** It reads only the config, as `/api/projects` does (`docs/canon/serve.md` "Concurrency").
- **Refusals.**
  - Any query parameter: 400, as for `check`, which takes none.
  - Another method: 405, `Allow: GET`.
  - A config `discover` refuses: 503 with the CLI's line.
- **On a hub:** mounted for every login, a plain read. It never reads `[team]`, and `GET_ROUTES` becomes 28.
- **Key sets.** `fixtures/daemon-keys.json` gains three, 110 to 113:
  - `WorkspaceDocument`: `source`, `notes`, `workspaces`, `modules`;
  - `Workspace`: `name`, `title`, `landing`, `sections`, `flows`, `module`;
  - `WorkspaceModule`: `name`, `title`, `root`.
- **07 s3** names the route before the UI calls it (`ui/README.md` "Contract seam"); this spec's change adds it there as "to come".

### The UI

- **Seam.**
  - `SpecEngineClient.getWorkspaces(project, signal?) -> Promise<WorkspaceDocument>`, with `HttpClient` and the mock.
  - `provisional.ts` types `WorkspaceDocument`, `Workspace` and `WorkspaceModule`. Each cites `docs/features/workspaces.md` "Data", a heading the shipped summary keeps.
  - `useWorkspaces(project)` in `queries.ts` is read again on window focus: a config edit raises no event.
- **The choice** (`src/workspace/choice.ts`).
  - The workspace key is `specengine.workspace.<p>`, holding a name. A stored name the document no longer serves gives the first workspace.
  - The module key is `specengine.module.<p>`. `""` stands for All modules. A served name is that module. No key gives the workspace's `module`.
  - Choosing a workspace removes the module key.
  - If storage refuses, the choice holds for the view until a reload, as `drift.ts` does.
- **The nav** (`routes.ts` `navOf(project, sections)`):
  - It lists Overview, then the chosen workspace's sections, each drawn only if `isSectionId`.
  - Pending: the nav is a skeleton with `aria-busy`.
  - The read failed: the nine sections, as today.
  - A current section the nav does not list: the view renders, and no item is `aria-current`.
- **The switcher.** A `<select>` named "Workspace", its options the titles in served order, with `WORKSPACE_TEXT.workspaceHint` as its description. It sits in the shell's header and shows only with two or more workspaces.
  - Choosing an option stores it, removes the module key, and pushes the landing's hash (`#/<p>` for `overview`): one history entry.
- **The landing.**
  - `#/` is replaced by the first project's landing hash once its workspaces read answers, or by its home if the read fails.
  - `#/<p>` renders the Overview.
- **The notes.** A disclosure "Config notes (n)" beside the switcher, or in its place with fewer than two workspaces. It lists each note verbatim, in served order. On a failed read it holds the error verbatim and Retry. With no note and no failure it is not drawn.
- **The module bar** (`src/workspace/ModuleBar.tsx`). It shows on the Tree and Flow views alone, when `modules` is not empty.
  - It is a `role="group"` named "Modules": "All modules", then each module's title, with `aria-pressed` on the chosen one. It wraps, with no sideways scroll at 320 px.
  - **Tree.** The view's root starts at the chosen module's root, or `null`, and resets to it when the choice changes. "Whole tree" and "Show X as root" still change the view's root alone.
  - **Flow.** The Top read is `{root: <root>, depth: 0, trace: true}` under a module and `{depth: 0, trace: true}` without one. A sole top group starts open as it does today. Group, Lanes, Lane S and Node R reads are unchanged.
- **The Board.**
  - **Tabs.** The workspace's flows, in its order, that the cards read serves. With `null`, or with none of them served, every flow is shown.
  - **Hidden flows.** When some are hidden, a line `WORKSPACE_TEXT.flowsShown(k, n)` and a button "Show all flows" show every tab for the visit. It turns into "Show this workspace's flows", and resets on a workspace change.
  - The Discipline and Assignee filters are unchanged.
- **Strings** (D12). `src/workspace/text.ts` exports `WORKSPACE_TEXT`:
  - `workspaceLabel` "Workspace";
  - `workspaceHint` "Orders the sections for one trade. It changes no right, and every section stays in the palette.";
  - `modulesLabel` "Modules", `allModules` "All modules";
  - `notesTitle` "Config notes", with the count shown as `(n)`;
  - `readFailed` "The workspaces could not be read: every section is shown.";
  - `flowsShown(shown, all)` "Showing <shown> of <all> card flows.";
  - `showAllFlows` "Show all flows", `showWorkspaceFlows` "Show this workspace's flows".
- **Mocks.**
  - harbor-sim serves `galley` and `foundry`:
    - `galley`: sections board, flow, tree, inbox; landing flow; flows copy; module berths;
    - `foundry`: sections board, health, graph, tree; flows plates.
  - Its modules: `berths` (root DOM-BERTHS) and `waters` (root DOM-WATER).
  - ledger-api serves the implicit document; `error` fails the read; one variant serves two notes.

### The gamedev preset (`scripts/presets/gamedev.toml`)

It holds the five card flows from the role research, with English titles, and four workspaces. It holds no `[[module]]`: roots are a project's own IDs. A project copies the file into its config, adds its modules and each workspace's `module`, and retitles anything in its own language. The lead's workspace is `production`, so that no name echoes `[team]`'s `lead` role (risk 10). Its card flow, also `production`, is named for the same reason.

```toml
# The gamedev preset (docs/features/workspaces.md "The gamedev preset"): five
# card flows and four workspaces for a game studio's trades. Copy it into a
# project's specengine.toml, add the project's own [[module]] tables and a
# workspace's `module`, and retitle anything in the project's language.
# Every name is data: no SpecEngine code reads one (ADR-0008, ADR-0073).

[[card.flow]]
name = "narrative"
disciplines = ["narrative"]
columns = ["backlog", "outline", "draft", "review", "approved", "ready-for-dev", "in-game"]
titles = { backlog = "Backlog", outline = "Outline", draft = "Draft", review = "Review", approved = "Approved", ready-for-dev = "Ready for dev", in-game = "In game" }
accept = { from = "ready-for-dev", to = "in-game" }

[[card.flow]]
name = "development"
disciplines = ["development"]
columns = ["backlog", "ready", "in-progress", "in-review", "verify", "done"]
titles = { backlog = "Backlog", ready = "Ready", in-progress = "In progress", in-review = "In review", verify = "Verify", done = "Done" }
accept = { from = "verify", to = "done" }

[[card.flow]]
name = "art"
disciplines = ["art"]
columns = ["waiting", "ready", "in-progress", "pending-review", "approved", "in-engine"]
titles = { waiting = "Waiting", ready = "Ready", in-progress = "In progress", pending-review = "Pending review", approved = "Approved", in-engine = "In engine" }
accept = { from = "approved", to = "in-engine" }

[[card.flow]]
name = "level-design"
disciplines = ["level-design"]
columns = ["layout", "blockout", "playtest", "scripting", "lighting", "art-pass", "final"]
titles = { layout = "Layout", blockout = "Blockout", playtest = "Playtest", scripting = "Scripting", lighting = "Lighting", art-pass = "Art pass", final = "Final" }
accept = { from = "art-pass", to = "final" }

[[card.flow]]
name = "production"
disciplines = ["production"]
columns = ["to-do", "in-progress", "review", "done"]
titles = { to-do = "To do", in-progress = "In progress", review = "Review", done = "Done" }
accept = { from = "review", to = "done" }

[[workspace]]
name = "writing"
title = "Writing"
sections = ["flow", "tree", "board", "questions", "inbox"]
landing = "flow"
flows = ["narrative"]

[[workspace]]
name = "development"
title = "Development"
sections = ["board", "inbox", "tree", "flow", "graph", "health", "roadmap", "questions"]
landing = "board"
flows = ["development"]

[[workspace]]
name = "art"
title = "Art and levels"
sections = ["board", "flow", "tree", "questions", "inbox"]
landing = "board"
flows = ["art", "level-design"]

[[workspace]]
name = "production"
title = "Production"
sections = ["inbox", "roadmap", "board", "flow", "tree", "graph", "health", "questions"]
```

No flow has `moves`: every ordered pair of open columns stands, so nothing is held (ADR-0012). The research's static columns, swimlanes, a Returned badge and an Unmapped column are step 3's (RM-0085).

### Fixtures

- **`fixtures/card-press`**, the non-game one (a print shop). The example tables above are appended at the end of its `specengine.toml`, so no line of its `[card]` moves. Its `expected.json` gains `"workspaces"`, the text output above.
- **`fixtures/studio-lamp`**, new: an invented lighthouse puzzle game, with no code.
  - **Config.** `[project] slug = "studio-lamp"`; `[ids]` `CHP = {kind = "chapter", shape = "name"}`, `SCN = {kind = "scene", shape = "name"}`, `ISL = {kind = "island", shape = "name"}`. Then the preset's bytes verbatim, between the lines `# --- scripts/presets/gamedev.toml ---` and `# --- end of the preset ---`. Then its own modules: `shore` (root CHP-SHORE) and `island` (root ISL-LAMP).
  - **Documents.** CHP-SHORE; SCN-ARRIVAL and SCN-STORM under it, SCN-STORM `follows` SCN-ARRIVAL; ISL-LAMP, with ISL-COVE under it.
  - **`expected.json`.** Prefixes, kinds and the `spec workspace` text.
- **Disjoint names.**
  - card-press: workspaces newsroom and pressroom; modules weekly and rulings; flows copy and plates.
  - studio-lamp: workspaces writing, development, art and production; modules shore and island; flows narrative, development, art, level-design and production.
  - The UI mocks: galley, foundry, berths and waters.
- **Kinds.** No kind is a word of an `http/src` literal: `chapter`, `scene` and `island` appear in none (risk 8). `no_domain.rs` reads every fixture's kinds, so a collision turns it red by itself.

## Rules and edge cases

- **W1** WHEN a table is malformed, the reader SHALL serve its default (the implicit workspace, or no module) with one note, and every command SHALL exit as without the table.
- **W2** WHEN a good table names an unknown section, flow, landing or module, the reader SHALL leave that value out with a note and serve the rest.
- **W3** The route SHALL read neither `[team]` nor the index, and SHALL take no turn.
- **W4** WHEN a section is not in the chosen workspace, its route, its palette entry and every link to it SHALL still render it, with no redirect.
- **W5** The UI SHALL branch on no workspace or module name, and the core SHALL name no trade. Names, titles and roots are data, shown or sent, never compared to a literal.
- **W6** WHEN the workspaces read is pending or fails, the nav SHALL be a skeleton, or the nine sections, never empty.

Edge cases:
- **A module's root naming nothing.** The Tree or Flow read answers its exit-1 `reason`, shown verbatim as the view's error. The bar stays, so another module can be chosen.
- **A look-alike or malformed root.** The read's exit 2 (a 503), shown verbatim.
- **One workspace configured.** There is no switcher, and its sections and landing apply.
- **A workspace whose every section is unknown.** The nav is Overview alone, plus the notes. It is not replaced by the implicit list, because the table itself is well formed.
- **Every flow unknown.** `flows` is `null` and the Board shows every flow, with the notes.
- **A later key** (RM-0089's per-login default, a module selector). On this build it is an unknown key: the table falls back with a note, and nothing is refused, because the tables are read apart.
- **`--config FILE`.** Notes name the file as given.
- **Mixed scripts in a title** are allowed: titles are data (ADR-0024 covers this repository, not a project's config).

## Acceptance criteria

CLI and http criteria run on scratch git copies of the named fixtures. UI criteria run on the mock through Testing Library, with a counting stub where reads are counted. M names the mutations that must turn each red.

- [ ] **WS-1 The implicit workspace.**
  - On `fixtures/spec-a`, `spec workspace --json` prints exactly the implicit document above, and the text reads `workspaces implicit | workspaces 1, modules 0`, then `workspace all | All sections | landing overview | sections inbox, board, roadmap, flow, tree, graph, health, questions | flows (all) | module (none)`.
  - `GET .../workspaces` is byte-equal to the JSON.
  - On the mock's ledger-api, the nav reads Overview, Inbox, Board, Roadmap, Flow, Spec tree, Graph, Health, Questions; there is no Workspace control and no module bar on Tree or Flow.
  - M1: `tasks` kept in `IMPLICIT_SECTIONS`. M2: the nav drawn from `SECTIONS`.
- [ ] **WS-2 Tasks is still a route.** On ledger-api:
  - `#/ledger-api/tasks/T-0107` renders the task's view;
  - the palette, typed "Tasks", lists the section;
  - the Inbox's link to `#/<p>/tasks` renders it;
  - ui-live-tasks AC-12 passes unchanged.
  - M3: `tasks` dropped from `isSectionId`. M4: the palette built from the workspace's sections.
- [ ] **WS-3 Choosing a workspace** (harbor-sim).
  - A first visit chooses `galley`, with the nav Overview, Board, Flow, Spec tree, Inbox.
  - Choosing Foundry:
    - sets `localStorage["specengine.workspace.harbor-sim"]` to `foundry`;
    - gives the nav Overview, Board, Health, Graph, Spec tree;
    - pushes `#/harbor-sim`, with `history.length` up by one.
  - A remount keeps `foundry`.
  - With `setItem` throwing, `foundry` holds until the remount.
  - M5: the choice in component state (the remount reads `galley`). M6: a key without the project.
- [ ] **WS-4 A bad table fails nothing** (card-press, one scratch change each; `<line>` is the offending value's line in that copy). Each variant expects:
  - (a) newsroom's `sections` gains `"stage"`: the unknown-section note, and newsroom keeps `board, tree, inbox`;
  - (b) newsroom's `flows` gains `"inks"`: the unknown-flow note;
  - (c) pressroom gains `landing = "inbox"`: the landing note, and `landing` reads `overview`;
  - (d) pressroom gains `module = "plates-room"`: the module note;
  - (e) newsroom's `name` becomes `"News room"`: `source` reads `implicit`, with one note ending `; the implicit workspace is served`, and both modules are still served;
  - (f) rulings loses `root`: `modules` is `[]`, one note ending `; no module is served`, and newsroom's module note; `source` stays `config`;
  - (g) copy's `accept` is removed: both `flows` lists are served as written, with one note ending `; the workspaces' flows are served unjudged`.

  In every variant:
  - `spec workspace` exits 0, its `note:` lines equal JSON `notes`, and `GET .../workspaces` is 200 and byte-equal to the JSON;
  - `spec check --json`'s `counts`, `spec tree --json` and `GET /api/projects` are byte-equal to the same copy's with both tables removed.

  M7: a malformed table refused by `ProjectConfig` (exit 2, and `/api/projects` 503). M8: (e) also dropping the modules. M9: (a) failing its workspace whole.
- [ ] **WS-5 A lens hides no route.** In `galley`, which lacks Health:
  - `#/harbor-sim/health` renders Health's `h1`, with no redirect, no history entry, and no nav item `aria-current`;
  - the palette lists the nine sections in `sections.ts` order.
  - M10: a redirect to the landing. M11: as M4.
- [ ] **WS-6 No name is code.**
  - (a) `crates/specengine-http/tests/no_domain.rs` adds to its words every `name` of a `[[workspace]]` or `[[module]]` in every repository `specengine.toml` and in `scripts/presets/*.toml`. A planted `"writing"` in a scratch source is red.
  - (b) A new `crates/specengine-eval/tests/workspace_names.rs` finds no string literal in `crates/*/src` or in `ui/src`'s app code (not `src/mocks/`, `src/test/` or `*.test.ts(x)`) that is exactly one of those names or a preset `[[card.flow]]` name.
  - (c) `ui/src/policy.test.ts` reads the mock's workspace and module names at run time and finds none as a literal in app code.
  - M12: a handler branching on `"writing"`. M13: `name === "newsroom"` in `Shell.tsx`. M14: `"narrative"` in core as a default. M15: `"galley"` in `BoardView.tsx`.
- [ ] **WS-7 The preset.**
  - `scripts/presets/gamedev.toml` is appended to `[project]\nslug = "preset"\n`. On that text:
    - `ProjectConfig` accepts it;
    - `WorkspaceConfig` reads `source` `config`, the four workspaces of "The gamedev preset" in order with its fields, and no note;
    - `CardConfig` reads its five flows with their columns, titles and `accept`.
  - `fixtures/studio-lamp/specengine.toml` holds the preset's bytes verbatim between its markers.
  - Its `spec workspace` equals `expected.json`, with no note.
  - `anonymity.rs`'s `AREAS` gains `scripts/presets`.
  - M16: a title changed in the fixture's copy alone. M17: `scripts/presets` out of `AREAS`, so a planted pilot name passes.
- [ ] **WS-8 Every command loads the tables.**
  - On card-press, each exits 0: `spec index`, `spec check` (0 errors), `spec tree`, `spec flow`, `spec card list`, `spec team`, `spec workspace`, `spec mcp`'s `initialize` and `tools/list`, and a daemon's start.
  - A copy whose first `[[workspace]]` header is misspelt `[[workspaces]]` makes each exit 2 with `specengine.toml:<line>: ` and the TOML reader's unknown-field message.
  - M18: `workspace` or `module` missing from `RawFile`.
- [ ] **WS-9 Modules root Tree and Flow** (harbor-sim, a counting stub).
  - In `galley`, the Tree's first read is `getTree(p, {root: "DOM-BERTHS", archive: false})` and Flow's Top read `{root: "DOM-BERTHS", depth: 0, trace: true}`.
  - All modules gives `{archive: false}` and `{depth: 0, trace: true}`, and stores `""`.
  - `waters` roots both at DOM-WATER, and a remount keeps it.
  - Choosing Foundry removes the module key, so both reads are unrooted.
  - The bar is a group named "Modules" with `aria-pressed` on the chosen. Board, Inbox and Health show no bar, and ledger-api shows none anywhere.
  - Group, Lanes, Lane S and Node R reads keep their pre-slice options.
  - M19: a bar made from the tree's top level (ledger-api shows one). M20: the module sent with the Board's read. M21: Flow left unrooted.
- [ ] **WS-10 A hub.**
  - On a hub over card-press whose `[team]` holds an unknown key, any login's `GET .../workspaces` is 200 and byte-equal to a local daemon's over the same copy. With `[team]` fixed, it is byte-equal again.
  - `GET_ROUTES` in `tests/hub.rs` pins 28, `/api/projects/{p}/workspaces` among them.
  - M22: the route reading `TeamConfig` (503). M23: the route left out of the pin.
- [ ] **WS-11 The landing** (harbor-sim).
  - `#/` is replaced, with no history entry, by `#/harbor-sim/flow` (`galley`'s landing) once the read answers.
  - With the read failing, `#/` goes to `#/harbor-sim`.
  - A typed `#/harbor-sim` renders the Overview.
  - M24: the landing ignored on `#/`. M25: `#/<p>` redirected.
- [ ] **WS-12 The route** (`tests/workspaces.rs`, `tests/turn.rs`).
  - `?x=1` gets 400 naming `x`; HEAD and POST get 405 with `Allow: GET`.
  - A `[project]` slug `ProjectConfig` refuses gets 503 with the CLI's line.
  - With the project's turn held as `turn.rs` holds it, the route answers 200 before the turn is released.
  - `daemon_keys.rs` asserts the three new sets.
  - M26: the handler awaiting the turn. M27: a key missing from a set.
- [ ] **WS-13 Notes and failures on screen.**
  - The two-note variant's "Config notes (2)" lists both notes verbatim.
  - `?scenario=error` gives the nine-section nav, with the error verbatim and Retry in the disclosure.
  - Pending, the nav is `aria-busy`.
  - M28: notes not drawn. M29: an empty nav on a failed read.
- [ ] **WS-14 The Board's flows** (harbor-sim). In `galley`:
  - there is no tab, and the line reads "Showing 1 of 2 card flows.";
  - "Show all flows" gives both tabs and "Show this workspace's flows";
  - switching to Foundry resets it.

  `null` flows show every tab and no line. M30: tabs that ignore the workspace.
- [ ] **WS-15 One place for the strings** (D12).
  - With `src/workspace/text.ts` mocked to sentinel values, the switcher, the bar, the notes and the Board's line render only sentinels, served titles, notes and numbers.
  - `sections.ts` still holds every section label.
  - M31: an inline `"Workspace"` label.
- [ ] **F Form.**
  - `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` are clean, and no dependency is added.
  - The default `cargo nextest run` passes.
  - `pnpm --dir ui test`, `lint`, `typecheck` and `build` pass.
  - `spec check` reports 0 errors and 0 warnings.

### Fixtures and tests to add

Fixtures (test-engineer):
- `fixtures/studio-lamp`, new;
- `fixtures/card-press`, with its tables appended once `RawFile` takes them;
- `fixtures/daemon-keys.json`, rewritten by `SPECENGINE_WRITE_DAEMON_KEYS=1`;
- `fixtures/spec-a`, unchanged.

Tests (test-engineer, Rust):
- core `tests/workspace_toml.rs`: the grammar, the limits at cap and cap + 1, every note's text, the implicit document, the preset (WS-7), and `IMPLICIT_SECTIONS` as `SECTIONS` without `tasks`;
- CLI `tests/workspace_cli.rs`: WS-1, WS-4, WS-7 and WS-8;
- http:
  - `tests/workspaces.rs`: WS-1's route, WS-4's routes and WS-12;
  - `tests/turn.rs`: WS-12's turn;
  - `tests/hub.rs`: WS-10's pin; `tests/hub_team.rs` or `tests/hub.rs`: WS-10's bad `[team]`;
  - `tests/no_domain.rs`: WS-6 (a);
  - `daemon_keys.rs`;
- eval:
  - `tests/workspace_names.rs`: WS-6 (b);
  - `tests/ui_policy.rs`: core `SECTIONS` equals `sections.ts`'s IDs in order (M: a section added to one side alone);
  - `tests/anonymity.rs`: WS-7.

Each cites `docs/features/workspaces.md` "Acceptance criteria".

Tests (ui-developer, under `ui/src`):
- `src/workspace/*.test.tsx`: WS-3, WS-9, WS-13 and WS-15;
- Shell and routes tests: WS-1, WS-2, WS-5 and WS-11;
- Board: WS-14;
- `policy.test.ts`: WS-6 (c).

## Out of scope

- A dashboard per workspace (RM-0086); static columns, swimlanes, a card modal and hand-offs (RM-0085).
- A hub login's default workspace (RM-0089); any read of `[team]`.
- Module filters on the Board and the Roadmap; a module in the URL hash (2b may put a scope there); a kind selector for modules (D4).
- An MCP tool or resource; a `spec check` finding for a bad table (the notes are the CLI's and the daemon's).
- `spec init --pack` and a config proposal (RM-0061); translating anything (RM-0082).
- The Overview's content; Tasks inside cards (step 3).

### Risks

- **R1 Older binaries.** `RawFile` is `deny_unknown_fields` (`crates/specengine-core/src/project_toml.rs`), so a `spec`, daemon or MCP server built before this step refuses a whole config holding either table. A hub at `v0.29.1` serving such a project fails every read with 503. The rule: **do not commit these tables to a hub-served project before its hub runs a build of this step**; every terminal and plugin upgrades first too. The gate's hook is unaffected (`docs/canon/gate.md`). Stated in "Canon at ship" as a known limit.
- **R4 Contract seam.** The types copy this spec's "Data" before canon holds it. The shipped summary keeps a "Data" heading pointing at `docs/canon/workspaces.md`, so their citations hold.
- **R8 Genre guard.** `module` is already a fixture kind, so no `http/src` literal may hold the word `module`; the JSON keys are the CLI's serde fields, not http literals. Section IDs reach http only as route paths.
- **R10 A lens taken for an identity.** D6's name and hint. The preset has no `lead` workspace or flow.
- **R-nav** The nav waits on one more read at first paint: a skeleton, not a flash of nine sections.
- **R-order** card-press's tables land only after `RawFile` takes them, or 218 test lines go red.
- **R-cap** `crates/specengine-core/README.md` has 7 B free, `crates/specengine-cli/README.md` 15 B, `crates/specengine-http/README.md` 6 B and `docs/canon/spec-flow-screen.md` 23 B. "Canon at ship" names where the room comes from.

## Canon at ship

Applied by the spec-writer when the slice ships. On each file touched, `reviewed:` moves to the ship date. Byte counts are against today's files.

- **`docs/canon/workspaces.md`** is rewritten whole, at most 8 KB:
  - "State" is dropped;
  - "The lens" is kept word for word;
  - then "Config" (the two grammar tables, the limits, the judging order and the notes' texts);
  - "Read" (`spec workspace`, the route, no turn, the key sets);
  - "Screen" (the nav, the switcher, the landing, the notes, the module bar, the Board's flows, the two keys, `WORKSPACE_TEXT`);
  - "Preset" (its path and its five flows and four workspaces as a table);
  - "Guards" (WS-6's three tests);
  - "Known limits": R1 word for word, one root per module, a choice per browser, no check finding;
  - "Tests".
- **`docs/specs/specengine-platform/07-interfaces.md`**:
  - s2's `spec workspace` line and s3's route lose "to come";
  - s3's screens paragraph gains "The nav is Overview, then the chosen workspace's sections (`docs/canon/workspaces.md`)", and the Tasks row gains "off the nav; a route and a palette entry";
  - s5 gains the card-press example as two short tables.
- **`ui/README.md`** (+229 B, 1 819 B free after):
  - "Screen rules", **Status**: "link types (but `mentions`)" becomes "link types (but `mentions`), workspace and module names";
  - **Shell**: "nine sections;" becomes "nine sections, each a route whatever the nav lists: Overview, then the chosen workspace's sections; `#/` and a switch open its landing (`workspaces.md` "Screen");"
  - and "`specengine.flow.gaps-hidden` and `specengine.board.as`" becomes "`specengine.flow.gaps-hidden`, `specengine.board.as`, `specengine.workspace.<p>` and `specengine.module.<p>`".
- **`crates/specengine-http/README.md`** (−8 B, 14 B free after):
  - "Endpoints": the identity row becomes "GET `/api/identity`, `.../workspaces`; a hub's `.../team`, ..." with answers "`hub.md`, `workspaces.md`, `hub-team.md`, `hub-git.md`" (+35);
  - "One door": "a configured kind or a task state as a word," becomes "a configured kind, a workspace or module name or a task state as a word," (+28);
  - "Tests" adds `workspaces.rs` after `roadmap.rs` (+17);
  - the room: "Known limits" moves its `EventSource` line (−88) to `docs/canon/serve-ui.md` "Known limits", word for word: it is a browser's limit.
- **`crates/specengine-cli/README.md`** (+5 B, 10 B free after):
  - "Commands": "`team` (`team.rs`): `hub-team.md`;" gains " `workspace`: `workspaces.md`;" (+30);
  - the room: "`verify …`, anchors: `spec-verify{,-output,-scope,-anchors}.md`; `verify --tests`, a terminal's: `spec-verify-tests.md`;" becomes "`verify …`, anchors, a terminal's `--tests`: `spec-verify{,-output,-scope,-anchors,-tests}.md`;" (−25).
- **`crates/specengine-core/README.md`** (−2 B, 9 B free after):
  - "API": "`CardConfig`, `card`: `cards.md`;" gains " `WorkspaceConfig`: `workspaces.md`;" (+36);
  - "`flow`, `card`, `team` only name-checked" becomes "`flow`, `card`, `team`, `workspace`, `module` only name-checked" (+23);
  - the room comes from detail already in Tier 2:
    - "`from_rows` (`GraphRows`, `{Edge,Parent}Row`)" becomes "`from_rows`" (−34, `spec-read-subset.md`);
    - "`revision` (`Revision`, `number()`)" becomes "`revision` (`Revision`)" (−12, `node-revision.md`);
    - "`bundle_node_from_toml` (`BundleNode`)" becomes "`bundle_node_from_toml`" (−15, `spec-cli-bundle.md`).
- **`docs/canon/serve.md` "Concurrency"**: "`/api/projects` and the tail's polls take no turn" becomes "`/api/projects`, `.../workspaces` and the tail's polls take no turn" (+18 B; 2 B free if local-landing's +151 B landed first).
- **`docs/canon/board-ui.md` "Screen"** (+111 B, 663 B free after):
  - "Section `board`, third of nine:" becomes "Section `board`:";
  - **Flows and filters**: "A tab per flow, none for one flow." becomes "A tab per flow the chosen workspace shows, in its order, none for one; past them `Showing <k> of <n> card flows.` and Show all flows (`workspaces.md` "Screen").".
- **`docs/canon/spec-flow-screen.md`** (+17 B, 6 B free after):
  - "Route": ", nav fourth of eight," is dropped (−21);
  - "Reads": the Top row's query becomes "`depth=0&trace=true`, `root=` a module's (`workspaces.md`)" (+38).
- **`docs/canon/spec-roadmap-screen.md` "Route and read"**: ", nav third of eight" is dropped (−20 B).
- **`CLAUDE.md`**:
  - "State" gains `workspaces` among v4's shipped items;
  - "Layout"'s `scripts/` line gains "and `scripts/presets/`, the packs a project copies".
- **`docs/specs/specengine-platform/08-roadmap.md`**: row 1a and RM-0081 marked shipped.
- **No change**: `docs/canon/architecture.md` (`#universal` and `#control` already hold the rule), `docs/canon/cards.md`, `docs/canon/hub-team.md`, `docs/canon/serve-ui.md` (but the moved line).

## Implementation

Filled in after implementation.
