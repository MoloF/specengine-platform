---
class: spec
status: draft
scope: [crates/specengine-cli, scripts, fixtures]
ref: RM-0061, v5 core slice 5 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 ("RM-0061 pack"); the orchestrator's working answers, the owner's review open
adrs: []
---

# pack: `spec init --pack` installs a preset, and a second, non-game preset (RM-0061)

## Why

A new project today gets one file from `spec init`: `[project]`, its slug and commented examples (`crates/specengine-cli/src/init.rs` `scaffold`). Departments and flows come from copying `scripts/presets/gamedev.toml` by hand, and that file has no `[project]`, so a copy must be merged by hand.

After this slice `spec init --pack <file>` writes a working config in one act: `[project]`, the slug, then the pack's bytes verbatim, checked before anything is written. A second preset, for a software product team, ships beside the game one. It shows that the departments are data and keeps the core domain-free: three guards read every preset's names against the code (ADR-0008).

No ADR. A pack is a file outside the crates, as the gamedev preset is (ADR-0073), read by path. A pack by embedded name would put trade words into the binary and need an ADR amending ADR-0073. The config proposal, a writer's type change reviewed as a proposal, is RM-0110 `config-proposal`, which needs its own ADR (`docs/canon/architecture.md#apply`).

## Description and interactions

### Decisions

- **P1. By path, never by name.** `--pack <PATH>` is a file path, resolved against the current directory and echoed as given. No preset is embedded in the binary.
- **P2. The file is the header plus the pack, verbatim.** It is `[project]`, `slug = "<slug>"`, a blank line, then the pack's bytes unchanged, comments included. The scaffold's comments are not written with a pack.
- **P3. Checked before any write.** The pack and the composed text go through the core readers. Each refusal exits 2 naming the pack's own line, and no file is created.
- **P4. Never a merge.** An existing `specengine.toml` is refused as today, with or without a pack.
- **P5. A second preset, for a software product team** (the owner's pick): `scripts/presets/software.toml`, with five flows and five workspaces. Its names were checked against the literal guard before they were chosen ("Data").
- **P6. A fixture per preset, copied verbatim.** `fixtures/plant-diary` holds the software preset between marker lines, as `studio-lamp` holds the gamedev one.
- **P7. The gamedev preset's header names `--pack`**, and studio-lamp's copy follows it byte for byte.
- **P8. CLI only.** No MCP tool, daemon route, UI or plugin change.

## Data

### Command

```text
spec init [--slug S] [--pack PATH]
```

Library: `InitRequest` gains `pub pack: Option<PathBuf>`, and `InitOutcome` gains `pub pack: Option<String>`, the path as given. JSON is `{path, slug, pack}`, with `pack` `null` without the flag. The text is `created specengine.toml with slug <slug>`, plus ` from <PATH>` with a pack. Without `--pack`, the file, the text and the exit are byte-equal to today's.

### The file written

```text
[project]
slug = "<slug>"

<the pack's bytes>
```

A pack line L is file line L + 3. Each message names `<PATH>:<L>`, never the file's line.

### Checks, in order (each exit 2, `spec: <message>`, nothing written)

1. Today's: `--config` refused, the root directory, the slug.
2. **Read**: a regular file, at most 1 048 576 bytes (`PACK_MAX_BYTES`), UTF-8:
   - ``--pack <PATH>: cannot read: <io error>; a pack is a file's path``
   - ``--pack <PATH>: <n> bytes; a pack is at most 1048576``
   - ``--pack <PATH>: not UTF-8``
3. **The pack alone**, read as TOML:
   - not TOML: `<PATH>:<L>: <the reader's message>`;
   - its first line that is neither blank nor a comment does not start with `[`: ``<PATH>:<L>: a pack starts with a table header; a key above it would join [project]``;
   - it holds `project`: ``<PATH>:<L>: a pack holds no [project]: spec init writes it``;
   - it holds `team`: ``<PATH>:<L>: a pack holds no [team]: a project's people are its own``.
4. **The composed text**, read by core `ProjectConfig::from_toml`, `scheme_from_toml`, `paths_from_toml` and `card_from_toml`, in that order, the first error refusing: `<PATH>:<L>: <its message>`, or `<PATH>: <its message>` when the reader gives no line.
5. **The workspaces**: `WorkspaceConfig::from_toml` on the composed text. Any note refuses, the first by line: `<PATH>:<L>: <the note's message>`.
6. Today's warning when a config above configures a project, then the exclusive create. An existing file gives `specengine.toml already exists; nothing was written`.

No reader here judges a table such as `[gate]`, `[roadmap]` or `[[check.rules]]`: it is written as given, and `spec check` judges it on its first run.

### `scripts/presets/software.toml`

Every name below was checked at ccc445d. Each flow and workspace name, each flow title and the return column `reopened`: none is a string literal in `crates/*/src` or in `ui/src` app code (`workspace_names.rs`), and none is a word of a literal in `crates/specengine-http/src` (`no_domain.rs`). The analyst's suspects `docs`, `review` and `layout` are literals; `design` and `qa` are not, but stay unused. The return column differs from gamedev's `sent-back`, so no reader can lean on one word.

```toml
# The software preset (docs/features/pack.md): five card flows and five
# workspaces for a software product team's departments. Install it in a new
# project with `spec init --pack <this file>`, or copy it into an existing
# project's specengine.toml; then add the project's own [ids], [[module]]
# tables and a workspace's `module`, and retitle anything in the project's
# language. Every name is data: no SpecEngine code reads one (ADR-0008,
# ADR-0073). A flow's title and return, and a [[card.split]] row, need a build
# that reads them on every terminal, plugin and hub serving the project, an
# older one refusing the key.

[[card.flow]]
name = "product"
title = "Product"
disciplines = ["product"]
columns = ["ideas", "reopened", "discovery", "specifying", "sign-off", "planned"]
titles = { ideas = "Ideas", reopened = "Reopened", discovery = "Discovery", specifying = "Specifying", sign-off = "Sign-off", planned = "Planned" }
accept = { from = "sign-off", to = "planned" }
return = "reopened"

[[card.flow]]
name = "experience"
title = "UX design"
disciplines = ["experience"]
columns = ["waiting", "reopened", "research", "wireframes", "prototype", "usability-test", "handed-off"]
titles = { waiting = "Waiting", reopened = "Reopened", research = "Research", wireframes = "Wireframes", prototype = "Prototype", usability-test = "Usability test", handed-off = "Handed off" }
accept = { from = "usability-test", to = "handed-off" }
return = "reopened"

[[card.flow]]
name = "engineering"
title = "Engineering"
disciplines = ["engineering"]
columns = ["backlog", "reopened", "ready", "in-progress", "code-review", "staging", "released"]
titles = { backlog = "Backlog", reopened = "Reopened", ready = "Ready", in-progress = "In progress", code-review = "Code review", staging = "Staging", released = "Released" }
accept = { from = "staging", to = "released" }
return = "reopened"

[[card.flow]]
name = "quality"
title = "Quality assurance"
disciplines = ["quality"]
columns = ["waiting", "reopened", "test-plan", "testing", "regression", "signed-off"]
titles = { waiting = "Waiting", reopened = "Reopened", test-plan = "Test plan", testing = "Testing", regression = "Regression", signed-off = "Signed off" }
accept = { from = "regression", to = "signed-off" }
return = "reopened"

[[card.flow]]
name = "documentation"
title = "Documentation"
disciplines = ["documentation"]
columns = ["backlog", "reopened", "drafting", "tech-review", "editing", "published"]
titles = { backlog = "Backlog", reopened = "Reopened", drafting = "Drafting", tech-review = "Tech review", editing = "Editing", published = "Published" }
accept = { from = "editing", to = "published" }
return = "reopened"

[[workspace]]
name = "product"
title = "Product"
sections = ["roadmap", "board", "tree", "flow", "questions", "inbox"]
landing = "roadmap"
flows = ["product"]

[[workspace]]
name = "experience"
title = "UX design"
sections = ["board", "flow", "tree", "questions", "inbox"]
landing = "board"
flows = ["experience"]

[[workspace]]
name = "engineering"
title = "Engineering and quality"
sections = ["board", "inbox", "tree", "flow", "graph", "health", "roadmap", "questions"]
landing = "board"
flows = ["engineering", "quality"]

[[workspace]]
name = "documentation"
title = "Documentation"
sections = ["board", "tree", "flow", "questions", "inbox"]
landing = "board"
flows = ["documentation"]

[[workspace]]
name = "leadership"
title = "Leadership"
sections = ["inbox", "roadmap", "board", "flow", "tree", "graph", "health", "questions"]

# A direction: a module chosen by node kind, with no root. Use your project's
# own kinds, and add it only once every terminal, plugin and hub runs a build
# that reads `kinds`.
# [[module]]
# name = "stories"
# title = "Stories"
# kinds = ["story"]

# A split template: a task on a document of this kind, or holding nodes of
# it, pre-selects these disciplines in the Split dialog, and each subtask
# targets the nodes of its kinds. It never refuses a split. Use your project's
# own kinds, and add it only once every terminal, plugin and hub runs a build
# that reads `[[card.split]]`.
# [[card.split]]
# kind = "epic"
# disciplines = ["product", "experience", "engineering", "quality", "documentation"]
```

The file is 4 231 bytes. The `return` column is second in each flow, never `accept`'s, as `card-flows.md` "Config" requires.

### The gamedev header

The preset's first comment paragraph becomes the following, the rest of the file unchanged:

```toml
# The gamedev preset (docs/features/workspaces.md "The gamedev preset"): six
# card flows and five workspaces for a game studio's trades. Install it in a
# new project with `spec init --pack <this file>`, or copy it into a project's
# specengine.toml; add the project's own [[module]] tables and a workspace's
# `module`, and retitle anything in the project's language.
```

`fixtures/studio-lamp/specengine.toml` takes the same lines between its markers in the same change, so WS-7 stays green.

### `fixtures/plant-diary`

An invented app that reminds a household to water its plants, with no code and no game word. `specengine.toml`, in order:
- a header comment like studio-lamp's;
- `[project]` `slug = "plant-diary"`;
- `[ids]` `EPC = { kind = "epic", shape = "name" }`, `STR = { kind = "story", shape = "name" }`;
- `# --- scripts/presets/software.toml ---`, the preset's bytes verbatim, `# --- end of the preset ---`;
- then its own tables:

```toml
[[module]]
name = "watering"
title = "Watering"
root = "EPC-WATERING"

[[card.split]]
kind = "epic"
disciplines = ["product", "experience", "engineering", "quality"]
```

Under `docs/spec/`, front-matter as studio-lamp's (`id`, `class: canon`, `title`, `parent`, `owner`, `reviewed`), with a short paragraph each:

| File | ID | Title | Parent | Links |
|---|---|---|---|---|
| `epc-watering.md` | EPC-WATERING | Watering reminders | - | - |
| `str-schedule.md` | STR-SCHEDULE | Set a watering schedule | EPC-WATERING | - |
| `str-snooze.md` | STR-SNOOZE | Snooze a reminder | EPC-WATERING | `depends_on: [STR-SCHEDULE]` |
| `epc-household.md` | EPC-HOUSEHOLD | Household sharing | - | - |
| `str-invite.md` | STR-INVITE | Invite a housemate | EPC-HOUSEHOLD | - |

`expected.json` holds:
- `prefixes` `["EPC", "STR"]` and `kinds` `["epic", "story"]`;
- `workspaces`, `spec workspace`'s text: `workspaces config | workspaces 5, modules 1`, then a line per workspace in the preset's order, then `module watering | Watering | root EPC-WATERING`. The `leadership` line reads `landing overview` and `flows (all)`, and every workspace has `module (none)`.

## Rules and edge cases

- WHEN any check fails, `spec init` SHALL write nothing. That includes no partial file: the exclusive create's cleanup holds as today.
- WHEN the pack is valid, the file's bytes SHALL equal the header plus the pack's bytes, with no newline added or removed.
- WHEN `--pack` is absent, `spec init` SHALL behave exactly as today, apart from the JSON's `pack: null`.
- A pack's `title`, `return` and `[[card.split]]` need a build that reads them; an older one refuses the key (`docs/canon/workspaces.md` "Known limits"). The presets' headers say so.
- Determinism: one pack and slug give one file; no clock or environment enters it.
- No core change: the readers are today's, and init calls them on the composed text.

## Acceptance criteria

Every criterion is a test that the named mutation turns red. The new test is `crates/specengine-cli/tests/init_pack.rs`, on scratch directories under a scratch `HOME`; a directory is git-initialised where a command needs it.

- [ ] PK1 — **The gamedev pack.** In a scratch `lamp-test`, `spec init --pack <repo>/scripts/presets/gamedev.toml --json` exits 0 with `{"path": "specengine.toml", "slug": "lamp-test", "pack": "<as given>"}`. The file's bytes are `[project]\nslug = "lamp-test"\n\n` plus the preset's bytes. `spec workspace` prints `workspaces config | workspaces 5, modules 0`, then studio-lamp `expected.json`'s five workspace lines, and no `note:`. `spec card list --json` serves the six flows in the preset's order with their titles and `return` `sent-back`. The text form prints `created specengine.toml with slug lamp-test from <as given>`. Mutation, red: the pack re-serialised (its comments lost).
- [ ] PK2 — **Refused before any write, at the pack's own line.** Each case exits 2 with the "Checks" message, and no `specengine.toml` exists after:
  - (a) a copy of the software preset with `[project]` at line 10;
  - (b) a key `color = "red"` at line 10, above the first table;
  - (c) a `[team]` table;
  - (d) the first flow's `return = "ideas"` (line 18): `` `return`: `ideas` is the first column, where a new card starts ``, named `<PATH>:18`;
  - (e) a workspace section `"wiki"` (line 59): the workspaces' note at `<PATH>:59`;
  - (f) a bad `[ids]` entry;
  - (g) not TOML;
  - (h) not UTF-8;
  - (i) 1 048 577 bytes;
  - (j) a directory.

  Mutations, each red: write, then check ((a)-(e) leave a file); the file's line reported (`:21` for (d)).
- [ ] PK3 — **The software preset and its fixture.**
  - (a) `fixtures/plant-diary/specengine.toml` holds `scripts/presets/software.toml` verbatim between its markers, once each.
  - (b) `spec check` on a copy exits 0 with 0 findings.
  - (c) `spec workspace` equals `expected.json` `workspaces`, with no note.
  - (d) `spec init --pack scripts/presets/software.toml` in a scratch directory gives 5 workspaces and `modules 0` with no note, and `card list` serves 5 flows, each `return` `reopened`.
  - (e) `spec card split --target EPC-WATERING --title "Watering reminders" --dry-run --json` plans 5 children in flow order. `product`, `experience`, `engineering` and `quality` are `suggested`, each targeting `["EPC-WATERING"]` from `kinds`; `documentation` is not suggested, targeting the parent's.
  - (f) The guards are green: `workspace_names.rs`, `no_domain.rs` (its pinned split set gains the row's four disciplines), `anonymity.rs` and `ui/src/policy.test.ts`.

  Mutations, each red: a `"quality"` literal added in `crates/specengine-core/src` (`workspace_names.rs`); a flow title changed in the fixture's copy alone ((a)).
- [ ] PK4 — **Never a merge.** With a `specengine.toml` present, `spec init` and `spec init --pack <a valid pack>` each exit 2 with `specengine.toml already exists; nothing was written`, and the file's bytes are unchanged. Mutation, red: the file overwritten.
- [ ] PK5 — **By path only.** In a directory with no file named `gamedev`, `spec init --pack gamedev` exits 2 with ``--pack gamedev: cannot read: …; a pack is a file's path`` and writes no file. With a file `gamedev` holding the software preset's bytes, it installs those bytes. Mutation, red: a preset embedded by name (`include_str!`), so `--pack gamedev` succeeds.
- [ ] PK6 — **No pack, no change.** `spec init` and `spec init --slug x` give the file, text and exit of the slice's base commit, and JSON `{path, slug, pack: null}`. `init.rs`'s pins move only by the `pack` key. Mutation, red: the scaffold dropped without a pack.
- [ ] PK7 — **The gamedev header.** `scripts/presets/gamedev.toml` opens with "The gamedev header"'s lines, and `workspace_toml.rs` WS-7 is green, studio-lamp holding it verbatim. Mutation, red: the preset edited alone.
- [ ] PK8 — **Gates.** `cargo nextest run` for `specengine-cli`, `-core`, `-http` and `-eval` is green, then the full run once. `doc_pointers`, `ui_policy`, `anonymity` and `spec check` pass, and `pnpm test` is green (`policy.test.ts`).

## Out of scope

Out: a merge into an existing config; the config proposal (RM-0110); a pack by name or embedded; a Russian pack; any MCP tool, route, UI or plugin change; judging `[gate]`, `[roadmap]` and the other tables at init.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-cli/src/init.rs`, `main.rs`, `lib.rs` (exports, if any); `scripts/presets/software.toml` (new, as "Data" gives it); `scripts/presets/gamedev.toml` (the header) |
| test-engineer | `crates/specengine-cli/tests/init_pack.rs` (new); `fixtures/plant-diary/` (new); `fixtures/studio-lamp/specengine.toml` (the header, in the same change as the preset's); `init.rs`'s JSON pins; `no_domain.rs`'s pinned split set |

Comments cite `docs/features/pack.md` "Acceptance criteria" with a criterion ID, or a canon heading (`doc_pointers`).

### Work order

1. Start after RM-0059 `intake-similar` ships: one Rust slice at a time.
2. The rust-developer writes the command and the two presets. The test-engineer overlaps: PK2 and PK5 first, then the fixture with PK3, PK1, PK4, PK6 and PK7. The gamedev header and studio-lamp's copy land together. Then the review.
3. Ship: the spec-writer ("Canon at ship").

### Risks

- **The literal guard** catches a name only when the test runs. A later slice adding a literal `"product"` or `"quality"` turns it red; that is the guard working, and the literal must go.
- **Older binaries** refuse a pack's `title`, `return` and `[[card.split]]` (the headers say so).
- **The fixture walk**: generic fixture tests (`edge_keys.rs`, `norm_recipe.rs`, `check_genre.rs`) read plant-diary too; it must be clean.

### Canon at ship

Free bytes now are in brackets. "Net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/presets.md` (new, Tier 2) | `spec init --pack`: the file, the checks, the messages; both presets and their flows and workspaces; the fixtures; the guards; Known limits (older binaries; tables not judged at init) |
| `docs/canon/workspaces.md` [14] | "Preset" becomes a pointer to `presets.md`, which frees bytes |
| `crates/specengine-cli/README.md` [1] | the `spec init` line gains `[--pack PATH]` and the pointer (net not positive) |
| `docs/canon/card-flows.md` [75] | `return`: the presets' return columns, a pointer (net not positive) |
| `CLAUDE.md`; 08 s2 Phase 7 "Owner's word, 2026-11-12" row 5; RM-0061 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner did not answer: P1, P2, P4, P5, and the config proposal split off (his word of 2026-11-12, RM-0110).

**The spec-writer's:**
- W1: a blank line between the slug and the pack;
- W2: `[team]` refused (Q-D);
- W3: a key above the first table refused;
- W4: any workspace note refuses;
- W5: 1 MiB at most;
- W6: messages in the pack's lines;
- W7: JSON `pack`;
- W8: the gamedev header names `--pack` (P7);
- W9: the return word `reopened`;
- W10: the fixture's split row, which leaves `documentation` unsuggested;
- W11: no scaffold with a pack.

**The owner's questions:**
- **Q-A.** Split the config proposal out? Answered by his word of 2026-11-12: RM-0110.
- **Q-B.** A pack by path or by name? Working answer: by path. The cost of by name: trade words inside the binary, an ADR, and every preset edit becoming a release.
- **Q-C.** Which non-game field? His pick: a software product team. Any field works if its names pass the guards.
- **Q-D.** May a pack carry `[team]`? Working answer: no. The cost of yes: a shared pack, once committed, could give a stranger's login lead rights on a hub.
