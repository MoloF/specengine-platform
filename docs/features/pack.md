---
class: spec
status: shipped
shipped: 2026-11-12
scope: [crates/specengine-cli, scripts, fixtures]
ref: RM-0061, v5 core slice 5 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 ("RM-0061 pack"); the orchestrator's working answers, the owner's review open
adrs: []
---

# pack: `spec init --pack` installs a preset, and a second, non-game preset (RM-0061)

## Why

A new project got one file from `spec init`: `[project]`, its slug and commented examples. Departments and flows came from copying `scripts/presets/gamedev.toml` by hand, and that file has no `[project]`, so a copy had to be merged by hand.

Now `spec init --pack <file>` writes a working config in one act: `[project]`, the slug, then the pack's bytes verbatim, checked before anything is written. A second preset, for a software product team, ships beside the game one. It shows that the departments are data and keeps the core domain-free: three guards read every preset's names against the code (ADR-0008).

No ADR. A pack is a file outside the crates, as the gamedev preset is (ADR-0073), read by path. A pack by embedded name would put trade words into the binary and need an ADR amending ADR-0073. The config proposal, a writer's type change reviewed as a proposal, is RM-0110 `config-proposal`, which needs its own ADR (`docs/canon/architecture.md#apply`).

## Description and interactions

Each decision is canon now, in `docs/canon/presets.md`: P1 by path, never by name, P2 the header plus the pack verbatim, P4 never a merge: "The command"; P3 checked before any write: "Checks"; P5 the software preset, P7 the gamedev header naming `--pack`: "The presets"; P6 a fixture per preset: "Fixtures"; P8 CLI only: its opening. The writer's W1 to W11 (the blank line, `[team]` and a key above the first table refused, any workspace note refusing, 1 MiB, the pack's own lines, JSON `pack`, the header, `reopened`, the fixture's split row, no scaffold) are in that canon too.

The owner's questions, open, their working answers built: **Q-A** the config proposal split off, his word of 2026-11-12 (RM-0110); **Q-B** by path (the cost of by name: trade words in the binary, an ADR, every preset edit a release); **Q-C** a software product team, his pick; **Q-D** no `[team]` in a pack ("Known limits").

## Data

In the canon: `presets.md` "The command" (flags, text, JSON, the file, the line mapping), "Checks" (every message, in order), "The presets" (both, their flows and workspaces), "Fixtures", "Guards"; `workspaces.md` "Preset", a pointer; `card-flows.md` "`return`"; the CLI README's `spec init` line. The software preset's bytes: `scripts/presets/software.toml`, 4 231 B.

### `fixtures/plant-diary`

`presets.md` "Fixtures": the software preset verbatim between markers, `[ids]` `EPC` and `STR`, the module `watering`, a split row of four disciplines, `expected.json`. Under `docs/spec/`:

| File | ID | Parent | Links |
|---|---|---|---|
| `epc-watering.md` | EPC-WATERING | - | - |
| `str-schedule.md` | STR-SCHEDULE, its section `## Quiet hours {#STR-SCHEDULE-QUIET}` | EPC-WATERING | - |
| `str-snooze.md` | STR-SNOOZE | EPC-WATERING | `depends_on: [STR-SCHEDULE]` |
| `epc-household.md` | EPC-HOUSEHOLD | - | - |
| `str-invite.md` | STR-INVITE | EPC-HOUSEHOLD | - |

The section, added by the test-engineer, gives the store's `own_text_oracle` more nodes than files and one nested node; under a story, it leaves the split's `epic` targets `EPC-WATERING` alone.

## Rules and edge cases

In the canon: `presets.md` "Checks", "Known limits". **Rollout**: a new `spec` binary only; no plugin version, MCP tool, route, schema step or backup format. An older binary refuses a pack's flow `title`, `return` and `[[card.split]]`, as it refuses them in a config.

## Acceptance criteria

Every criterion is a test that the named mutation turns red. The new test is `crates/specengine-cli/tests/init_pack.rs`, on scratch directories under a scratch `HOME`; a directory is git-initialised where a command needs it.

- [x] PK1 — **The gamedev pack.** In a scratch `lamp-test`, `spec init --pack <repo>/scripts/presets/gamedev.toml --json` exits 0 with `{"path": "specengine.toml", "slug": "lamp-test", "pack": "<as given>"}`. The file's bytes are `[project]\nslug = "lamp-test"\n\n` plus the preset's bytes. `spec workspace` prints `workspaces config | workspaces 5, modules 0`, then studio-lamp `expected.json`'s five workspace lines, and no `note:`. `spec card list --json` serves the six flows in the preset's order with their titles and `return` `sent-back`. The text form prints `created specengine.toml with slug lamp-test from <as given>`. Mutation, red: the pack re-serialised (its comments lost).
- [x] PK2 — **Refused before any write, at the pack's own line.** Each case exits 2 with the "Checks" message, and no `specengine.toml` exists after: (a) a copy of the software preset with `[project]` at line 10; (b) a key `color = "red"` at line 10, above the first table; (c) a `[team]` table; (d) the first flow's `return = "ideas"` (line 18): `` `return`: `ideas` is the first column, where a new card starts ``, named `<PATH>:18`; (e) a workspace section `"wiki"` (line 59): the workspaces' note at `<PATH>:59`; (f) a bad `[ids]` entry; (g) not TOML; (h) not UTF-8; (i) 1 048 577 bytes; (j) a directory. Mutations, each red: write, then check ((a)-(e) leave a file); the file's line reported (`:21` for (d)).
- [x] PK3 — **The software preset and its fixture.** (a) `fixtures/plant-diary/specengine.toml` holds `scripts/presets/software.toml` verbatim between its markers, once each. (b) `spec check` on a copy exits 0 with 0 findings. (c) `spec workspace` equals `expected.json` `workspaces`, with no note. (d) `spec init --pack scripts/presets/software.toml` in a scratch directory gives 5 workspaces and `modules 0` with no note, and `card list` serves 5 flows, each `return` `reopened`. (e) `spec card split --target EPC-WATERING --title "Watering reminders" --discipline product --dry-run --json` (the first flow's discipline; without one the split exits 1, "no flow takes a card without a discipline") plans 5 children in flow order: `product`, `experience`, `engineering` and `quality` `suggested`, each targeting `["EPC-WATERING"]` from `kinds`; `documentation` not suggested, targeting the parent's. (f) The guards are green: `workspace_names.rs`, `no_domain.rs` (its pinned split set gains the row's four disciplines, and BH-13's pinned return columns gain `reopened`), `anonymity.rs` and `ui/src/policy.test.ts`. Mutations, each red: a `"quality"` literal added in `crates/specengine-core/src` (`workspace_names.rs`); a flow title changed in the fixture's copy alone ((a)).
- [x] PK4 — **Never a merge.** With a `specengine.toml` present, `spec init` and `spec init --pack <a valid pack>` each exit 2 with `specengine.toml already exists; nothing was written`, and the file's bytes are unchanged. Mutation, red: the file overwritten.
- [x] PK5 — **By path only.** In a directory with no file named `gamedev`, `spec init --pack gamedev` exits 2 with ``--pack gamedev: cannot read: …; a pack is a file's path`` and writes no file. With a file `gamedev` holding the software preset's bytes, it installs those bytes. Mutation, red: a preset embedded by name (`include_str!`), so `--pack gamedev` succeeds.
- [x] PK6 — **No pack, no change.** `spec init` and `spec init --slug x` give the file, text and exit of the slice's base commit, and JSON `{path, slug, pack: null}`. `init.rs`'s pins move only by the `pack` key. Mutation, red: the scaffold dropped without a pack.
- [x] PK7 — **The gamedev header.** `scripts/presets/gamedev.toml` opens with the five header lines naming `--pack`, and `workspace_toml.rs` WS-7 is green, studio-lamp holding it verbatim. Mutation, red: the preset edited alone.
- [x] PK8 — **Gates.** `cargo nextest run` for `specengine-cli`, `-core`, `-http` and `-eval` is green, then the full run once. `doc_pointers`, `ui_policy`, `anonymity` and `spec check` pass, and `pnpm test` is green (`policy.test.ts`).

## Out of scope

Out: a merge into an existing config; the config proposal (RM-0110); a pack by name or embedded; a Russian pack; any MCP tool, route, UI or plugin change; judging `[gate]`, `[roadmap]` and the other tables at init.

## Implementation

Rust review ACCEPT, its asks settled in iteration 2. PK8 is the orchestrator's full run, ticked on his word.

| Module | Does |
|---|---|
| cli `src/init.rs` | `InitRequest.pack`, `InitOutcome.pack`, `PACK_MAX_BYTES`; `read_pack` over `open_pack` (Unix `O_NONBLOCK`), `check_pack`, `check_composed` (`HEADER_LINES` 3), `refusal`, `line_of`; ` from <PATH>`, JSON `pack` |
| cli `src/main.rs`, `src/lib.rs` | `Init { slug, pack }`, `--pack PATH`; `PACK_MAX_BYTES` exported |
| `scripts/presets/software.toml` (new), `gamedev.toml` | the preset, byte-equal to the draft's block; the header's lines 1-4 replaced by five |
| cli `tests/init_pack.rs` (new), `tests/init.rs`, `tests/store_reads.rs`, `src/store_reads_tests.rs`; http `tests/no_domain.rs`; `fixtures/plant-diary` (new), `fixtures/studio-lamp/specengine.toml` | PK1-PK7; `pack: null`; plant-diary after the golden and in the differential; the split set and BH-13's return columns; the fixture; the header's copy |
| store `tests/projection.rs`, `tests/store_reads.rs`; eval `tests/import_support/mod.rs` | the configured-fixture premise 21 to 22; plant-diary on the import allowlist |

**Departures**, accepted by the review, canon now: (1) only the gamedev header's lines 1-4 are replaced, so the four after them, the data rule and the older-build warning, stay; (2) `ProjectConfig::from_toml` already runs the `[ids]` and `[paths]` readers, so init calls it, `card_from_toml` and `WorkspaceConfig::from_toml`, the same result; (3) a directory or other non-regular file: `cannot read: not a regular file; a pack is a file's path`, a wording the spec lacked; (4) a leading byte-order mark: at first the first-line check skipped it and the composed read refused it at line 1 with the TOML reader's message; iteration 2 refuses it first, with its own; (5) a reader's line inside the header gives `<PATH>: <message>`, unreachable while `project` is refused.

**Iteration 2**, the review's asks: the BOM refused first, `<PATH>:1: a pack starts with a byte-order mark; save it as UTF-8 without one`; the open non-blocking on Unix, `is_file` and the size cap judged on the open handle, not a look before it, so a FIFO, a symlink to one or a directory returns at once. Init and `init_pack` 29 of 29, clippy clean. The reviewer's probes passed: LF, CRLF, no final newline and a symlinked pack byte-equal; `["project"]`, `[project.extra]`, `[team.leads]`, `[[team]]` at their lines; a FIFO and `/dev/zero` behind a symlink, no hang; 1 048 576 bytes in, 1 048 577 out; deep nesting exit 2; an existing config or dangling symlink unchanged. An empty or comment-only pack writes only the header (and its comments).

**Exact lines**, the developer's runs, packs at `../packs/`, each exit 2 with no file:

```text
(a) spec: ../packs/a.toml:10: a pack holds no [project]: spec init writes it
(b) spec: ../packs/b.toml:10: a pack starts with a table header; a key above it would join [project]
(c) spec: ../packs/c.toml:106: a pack holds no [team]: a project's people are its own
(d) spec: ../packs/d.toml:18: `return`: `ideas` is the first column, where a new card starts
(e) spec: ../packs/e.toml:59: workspace `product`: `wiki` is no section (inbox, tasks, board, roadmap, flow, tree, graph, health, questions): left out
(f) spec: ../packs/f.toml:2: `EPC`: shape `nope` is neither "number" nor "name"
(g) spec: ../packs/g.toml:106: unclosed array table, expected `]`
(h) spec: --pack ../packs/h.toml: not UTF-8
(i) spec: --pack ../packs/i.toml: 1048577 bytes; a pack is at most 1048576
(j) spec: --pack ../packs/j: cannot read: not a regular file; a pack is a file's path
PK5 spec: --pack gamedev: cannot read: No such file or directory (os error 2); a pack is a file's path
PK4 spec: specengine.toml already exists; nothing was written
```

PK1, PK3 (d) and PK6 printed as their criteria say.

**Tests**: `init_pack.rs` 20 tests; the full workspace run 5 690, two gate timing tests flaky under load and green alone; 11 named mutations red. Pins moved: CLI `store_reads_tests.rs` `FIXTURES` 21 to 22, `store_reads.rs` `AFTER_THE_GOLDEN`; store `projection.rs`, `store_reads.rs` premise 21 to 22; eval `import_support` allowlist `fixtures/plant-diary/`. **Test differences**: PK3 (e) adds `--discipline product`, the draft's command exiting 1; `STR-SCHEDULE-QUIET` added to the fixture.
