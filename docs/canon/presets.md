---
class: canon
tier: 2
scope: [crates/specengine-cli, scripts, fixtures]
owner: owner
reviewed: 2026-11-12
---

# Presets: `spec init` and the packs it installs

A pack is a file of config tables outside the crates: a trade's card flows and workspaces, as data (ADR-0008, ADR-0073). `spec init --pack <PATH>` installs one in a new project in one write; a project may also copy one into its config. Two ship in `scripts/presets/`, `gamedev.toml` and `software.toml`. Built by `pack` (RM-0061), no ADR; a writer's config change reviewed as a proposal is RM-0110's, with its own ADR. CLI `init.rs`; no MCP tool, route, UI or plugin surface.

## The command

```text
spec init [--slug S] [--pack PATH]
```

It creates exactly one file, `specengine.toml`, at `--root` or the current directory (no walk up), exclusively: one there, or anything by that name, a dangling symlink too, gives `specengine.toml already exists; nothing was written`, the file unchanged, with or without a pack. Never a merge. A failed write removes the file it created.

- **Slug**: `--slug`, else the directory name, ASCII letters and digits lower-cased, every other run `-`, trimmed (`derive_slug`); core `slug_problem` -> exit 2 naming `--slug`. `--config` is refused. A config in an ancestor -> a `warning:`, the write still made.
- **Without `--pack`**: `[project]\nslug = "<slug>"\n`, then the scaffold's comments only (`[ids]`, `[[flow.start]]`, `[[flow.end]]` examples).
- **`--pack PATH`**: a file's path, resolved against the current directory and echoed as given; never a name, no preset embedded in the binary (`--pack gamedev` reads a file `gamedev`). No scaffold.
- **Output**: `created specengine.toml with slug <slug>`, with a pack ` from <PATH>`. JSON `{path, slug, pack}`, `pack` `null` without the flag. Library: `InitRequest {slug, pack: Option<PathBuf>}`, `InitOutcome.pack: Option<String>`, `PACK_MAX_BYTES` exported.

The file written with a pack:

```text
[project]
slug = "<slug>"

<the pack's bytes>
```

The pack's bytes verbatim: comments, CRLF, no final newline, a symlink's target's bytes; nothing added or removed. A pack line L is file line L + 3; every message names `<PATH>:<L>`, never the file's line. An empty pack writes only the 3-line header; a comment-only one, the header and its comments. Both are accepted, configuring nothing beyond `[project]`.

## Checks

In order, before any write; each exits 2 with `spec: <message>` and creates nothing (`init.rs` `read_pack`, `check_pack`, `check_composed`).

1. **As without a pack**: `--config`, the root directory, the slug.
2. **The read, judged on the opened file.** On Unix the open takes `O_NONBLOCK`, and `is_file` and the size come from the open handle, so a FIFO, a symlink to one, a directory or a device returns at once, and a file swapped in after a look is still judged; off Unix a plain open. At most `PACK_MAX_BYTES` (1 048 576) + 1 bytes are read, then UTF-8, then no leading byte-order mark:
   - ``--pack <PATH>: cannot read: <io error>; a pack is a file's path`` (`No such file or directory (os error 2)`);
   - ``--pack <PATH>: cannot read: not a regular file; a pack is a file's path``;
   - ``--pack <PATH>: <n> bytes; a pack is at most 1048576``, `<n>` the handle's size, or the bytes read when it grew;
   - ``--pack <PATH>: not UTF-8``;
   - ``<PATH>:1: a pack starts with a byte-order mark; save it as UTF-8 without one``.
3. **The pack alone**, as TOML (`toml`'s `DeTable`):
   - not TOML: `<PATH>:<L>: <the reader's message>`, or `<PATH>: <message>` with no span;
   - its first line neither blank nor a comment, trimmed, not opening with `[`: ``<PATH>:<L>: a pack starts with a table header; a key above it would join [project]``;
   - a top-level `project` (`[project]`, `["project"]`, `[project.x]`): ``<PATH>:<L>: a pack holds no [project]: spec init writes it``;
   - a top-level `team` (`[team]`, `[[team]]`, `[team.leads]`): ``<PATH>:<L>: a pack holds no [team]: a project's people are its own``. `project` is judged first, whatever the lines.
4. **The composed text**, through core: `ProjectConfig::from_toml`, which itself runs the `[ids]` reader (`scheme_from_toml`) and then the `[paths]` reader (`paths_from_toml`), so neither is called again; then `card_from_toml`. The first error refuses: `<PATH>:<L>: <its message>`, or `<PATH>: <its message>` when the reader gives no line or one inside the header (unreachable while `project` is refused). An unknown top-level table: ``<PATH>:<L>: unknown field `x`, expected one of `project`, …``.
5. **The workspaces**: `WorkspaceConfig::from_toml` on the composed text; its first note by line refuses, `<PATH>:<L>: <the note's message>` (`workspaces.md` "Judging").
6. The ancestor warning, then the exclusive create.

A table those readers only name-check (`RawFile`), such as `[gate]`, `[roadmap]` or `[check]`, is written as given; `spec check` judges it on its first run.

## The presets

Each is a header comment, `[[card.flow]]` tables, `[[workspace]]` tables, then commented examples; no `[project]`, `[team]`, `[ids]`, live `[[module]]` or live `[[card.split]]` row (roots and kinds are a project's). Each flow is its own discipline, `accept` its last two columns, no `moves`, its `return` column second (`card-flows.md` "Config"). The header says to install it with `spec init --pack <this file>` or copy it, to add modules, a workspace's `module` and titles in the project's language, that every name is data (ADR-0008, ADR-0073), and that a flow's `title` and `return` and a `[[card.split]]` row need a build that reads them on every terminal, plugin and hub serving the project.

**`gamedev.toml`**, a game studio's trades; no name echoes `[team]`'s `lead`. Header lines 1-5 name `--pack` (they replaced the old lines 1-4 alone); lines 6-9 keep the data and build rules. Six flows, `narrative` "Narrative", `development` "Programming", `art` "Art", `level-design` "Level design", `production` "Production", `sound` "Sound", the role research's columns, the return column `sent-back` "Returned". After the workspaces, a commented direction `scenes` and two commented `[[card.split]]` rows (`card-split.md`).

| Workspace | Sections | Landing | Flows |
|---|---|---|---|
| `writing` | flow, tree, board, questions, inbox | flow | narrative |
| `development` | board, inbox, tree, flow, graph, health, roadmap, questions | board | development |
| `art` "Art and levels" | board, flow, tree, questions, inbox | board | art, level-design |
| `sound` | board, flow, tree, questions, inbox | board | sound |
| `production` | inbox, roadmap, board, flow, tree, graph, health, questions | overview | all |

**`software.toml`**, a software product team's departments, 4 231 bytes. Five flows, the return column `reopened` "Reopened", unlike gamedev's so no reader leans on one word; names that are code literals (`docs`, `review`, `layout`) unused:

| Flow | Title | Columns | Accept |
|---|---|---|---|
| `product` | Product | ideas, reopened, discovery, specifying, sign-off, planned | sign-off -> planned |
| `experience` | UX design | waiting, reopened, research, wireframes, prototype, usability-test, handed-off | usability-test -> handed-off |
| `engineering` | Engineering | backlog, reopened, ready, in-progress, code-review, staging, released | staging -> released |
| `quality` | Quality assurance | waiting, reopened, test-plan, testing, regression, signed-off | regression -> signed-off |
| `documentation` | Documentation | backlog, reopened, drafting, tech-review, editing, published | editing -> published |

| Workspace | Sections | Landing | Flows |
|---|---|---|---|
| `product` | roadmap, board, tree, flow, questions, inbox | roadmap | product |
| `experience` "UX design" | board, flow, tree, questions, inbox | board | experience |
| `engineering` "Engineering and quality" | board, inbox, tree, flow, graph, health, roadmap, questions | board | engineering, quality |
| `documentation` | board, tree, flow, questions, inbox | board | documentation |
| `leadership` | inbox, roadmap, board, flow, tree, graph, health, questions | overview | all |

After them, a commented direction `stories` (`kinds = ["story"]`) and a commented `[[card.split]]` row, `epic`, all five disciplines.

## Fixtures

Each preset is copied byte for byte into a fixture between `# --- scripts/presets/<name>.toml ---` and `# --- end of the preset ---`, once each, so a preset edited alone turns a test red:

- `fixtures/studio-lamp`, a lighthouse puzzle game: gamedev (core `workspace_toml.rs` WS-7).
- `fixtures/plant-diary`, a household's plant-watering app, no code and no game word: `[ids]` `EPC` epic, `STR` story, `shape = "name"`; software; then `[[module]]` `watering` rooted at `EPC-WATERING`, and a `[[card.split]]` row, `epic`, `[product, experience, engineering, quality]`, leaving `documentation` unsuggested. Five documents: `EPC-WATERING` over `STR-SCHEDULE` (its section `STR-SCHEDULE-QUIET`, so the store's `own_text_oracle` has more nodes than files and one nested) and `STR-SNOOZE` (`depends_on: [STR-SCHEDULE]`), `EPC-HOUSEHOLD` over `STR-INVITE`. A split on `EPC-WATERING` needs `--discipline` (`card-split.md`); its `epic` targets stay `EPC-WATERING` alone. `expected.json`: prefixes, kinds and `spec workspace`'s text (`workspaces 5, modules 1`). The CLI's in-crate store differential lists it (`store_reads_tests.rs`, 22 fixtures), `store_reads.rs` keeps it out of the golden (`AFTER_THE_GOLDEN`), the store's `projection.rs` and `store_reads.rs` count it (22), and eval `import_support` allows it.

## Guards

`workspaces.md` "Guards" reads every preset as it reads a config. On this repository http `no_domain.rs` pins the split disciplines to `{copy, plates}` (card-press) plus plant-diary's four, and the presets' return columns (`board_handoff_bh13_…`) to `["reopened", "sent-back"]`. A crate literal `"product"` or `"quality"` turns eval `workspace_names.rs` red: the literal goes, not the preset.

## Known limits

- **Older binaries** refuse a pack's flow `title`, `return` and `[[card.split]]` (`workspaces.md` "Known limits"); the headers say so.
- **By path only**: a preset edit is a file edit, never a release.
- **No `[team]`**: a committed shared pack could give a stranger's login rights on a hub; people are added by hand.
- Other tables are only name-checked at init (step 4). Off Unix the open has no `O_NONBLOCK`.

## Tests

CLI `tests/init_pack.rs` (PK1-PK7 of `docs/features/pack.md`: the install verbatim, each refusal at the pack's line, a FIFO, a symlink to one and a directory at once, the BOM, the order, never a merge, by path, no pack no change, the header), `tests/init.rs` (`pack: null`); core `workspace_toml.rs` WS-7; http `no_domain.rs`; eval `workspace_names.rs`, `anonymity.rs`; the fixtures above.
