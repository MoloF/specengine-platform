---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, plugin, ui]
ref: RM-0111; the owner's goal of 2026-11-13 (writers see what is used where, by which documents and in which places, and orphan documents show up); the analyst's analysis of 2026-11-13, slice 1, with the owner's answers (Q1, an orphan is linked to nothing; Q2 a working answer); an earlier prototype's "filled by" panel
adrs: []
---

# usage-orphans: what uses a document, where, and which documents nothing links

## Why

Writers describe documents and reuse props and mechanics through links. Today they cannot see what uses a document without reading every In row of its links one by one. No list carries a usage count. Nothing names the documents that are linked to nothing, which are the ones to delete or bring in later. Flow's `definition-unused` counts only `uses`, and only on `[[flow.definition]]` documents.

This slice serves one count, computed per read and never stored, in four places. The document view shows who uses a document, grouped by kind, with each place. The direction catalog gets a "Used by" column and a "Used by 0" filter. Health gets an `orphans` row over the whole corpus. `get_node` serves the same numbers to agents. `corpus-map` (RM-0112) reuses the count and the orphan flag.

## Description and interactions

### Decisions

- **D1. One count, in the core, served everywhere.** Core `usage.rs` computes `used_by`, `mentioned_by` and `orphan` from the resolved graph. The catalog, Health and `show --links` serve its numbers, and so does `corpus-map`'s map. The UI never counts uses (`docs/canon/architecture.md#ui`); it only groups the served In rows it already lists.
- **D2. A use is a resolved link from another file, counted by its source.** The source is the innermost ID section around the link, else the document, else the file's path (`spec-cli-graph.md` "Parents and links"). Distinct sources are counted, not rows. A use of a section is also a use of every node that holds it. `parent:` is containment, never a use.
- **D3. An orphan is linked to nothing** (the owner's Q1): a live document with no use, no mention, no resolved link out to another file and no child. Only documents are judged; a section's `orphan` is `null`. Each section's "unused" shows as "Used by 0" in the catalog instead.
- **D4. Set aside and planned stay apart** (Q2, a working answer). `status: abandoned`, the convention's word for set aside (`documentation-system.md` §6), is never an orphan. It is set through a proposal, as any status is. An open card on an orphan means "bring it in later": the row stays and shows its tasks. No new state, store or ADR.
- **D5. Nothing is stored.** No table, column, `INDEX_FORMAT`, queue schema, backup format, `CHECK_CODES` entry or config key changes. An orphan is no check finding and blocks nothing (ADR-0012).
- **D6. `get_node` serves the count**, so an agent reads the number the writer sees. That is a key a tool returns: plugin 0.46.0 (`plugin-files.md` "Version"). Rejected: the UI counting rows, which would put the rule in TypeScript.
- **D7. `show --links` counts from the rows it already reads.** Its subset read holds every row ending on the node and its sections (`spec-read-subset.md` "The subset"), so `subset.rs` does not change. It serves no `orphan`: that needs children, a whole-graph read.
- **D8. Health judges every live document**, wherever it sits, `[paths] spec` or not. It serves rows by kind. The UI groups them by direction, then kind, as Inside does (`directions.md` "Inside"). Tasks come from the queue on a best-effort basis: a queue that does not read leaves `tasks` `null` with a note and never fails the row.

### What the owner sees

- **Document view, Links.** Under In, a line reads "Used by 2 · Mentioned by 6". Then come the users, grouped by kind. Each group shows its count, then each user with every place it cites from (`path:line`, and `at <section>` when the link lands on a section). At zero uses the line reads "Declared, nothing uses it yet."
- **Spec tree, a direction's table.** It gets a "Used by" column and an "Orphan" badge. A "Used by 0" filter keeps the unused rows.
- **Health, "What is left".** A new part, "Orphan documents", lists every orphan grouped by direction, then kind. Each row is a document link with its place and tasks. A task on a row means someone plans to bring it in.

## Data

### Core (`crates/specengine-core/src/usage.rs`, new)

```rust
/// One node's uses, per read, never stored (docs/features/usage-orphans.md "Acceptance criteria").
pub struct NodeUsage { pub used_by: u32, pub mentioned_by: u32, pub orphan: Option<bool> }
/// Every node of every live file of the graph, keyed by place.
pub fn usage_of(graph: &SpecGraph) -> BTreeMap<NodeAt, NodeUsage>;
/// The two counts from one node's incoming rows: `(used_by, mentioned_by)`.
pub fn tally<'a>(rows: impl IntoIterator<Item = UseRow<'a>>, own_file: &str) -> (u32, u32);
pub struct UseRow<'a> { pub source: &'a str, pub file: &'a str, pub weak: bool, pub live: bool, pub resolved: bool }
```

The rules, in order:
1. A **use of X** is a link that meets three conditions. It is `resolved`. It ends on X or on a node nested in X; for a document, that means any of its ID sections. It is written in a live file other than X's (`is_live`: neither Tier 3 nor `class: generated`), whatever `--archive` says.
2. `used_by` counts distinct sources over strong types. `mentioned_by` counts the same over `mentions` (`is_weak_link`). A source can be counted in both.
3. **Orphan**, for a live document D, means all of these hold:
   - D's `status:` is not `abandoned`;
   - both counts are 0;
   - no resolved link written in D's span (front-matter, body, every nested section) ends on a node of another file, whatever that file's standing;
   - no live node of another file has a resolved `parent:` ending on D or on one of D's sections.

   D's own `parent:` is not a link. A section gets `None`.

The core reads no kind, link type or class name. It reads the convention's `abandoned` and the model's `is_weak_link`.

**On `fixtures/spec-a`** (12 live documents; DEC-0007 is Tier 3):

| Node | `used_by` | `mentioned_by` | `orphan` | Why |
|---|---|---|---|---|
| MEC-STAMINA | 2 | 6 | false | DEC-0023 and MEC-SPRINT use it, the two through RULE-STAM-REGEN; Q-031 mentions it twice, counted once |
| RULE-STAM-REGEN | 2 | 3 | `null` | a section |
| MEC-SPRINT | 1 | 1 | false | |
| DOM-MOVEMENT | 0 | 1 | false | children |
| DOM-GAME | 0 | 0 | false | links out; a child |
| A-101 | 2 | 3 | false | |
| A-102 | 0 | 0 | **true** | no link either way, no child |
| DEC-0023 | 1 | 1 | false | DEC-0007's `supersedes` is Tier 3 and does not count |
| Q-031 | 1 | 3 | false | |
| Q-032 | 0 | 0 | false | links out (`working_answer`) |
| R-12 | 1 | 6 | false | `stamina-tuning.md` and its AC-07 are two sources |
| TERM-exhausted | 1 | 1 | false | |
| `docs/features/stamina-tuning.md` | 0 | 1 | false | its own AC-07's mention is the same file |

### `spec show REF --links`

The links block's last line gains the counts:

```text
  links 5 out, 10 in; used by 2, mentioned by 6
```

The `; left out: …` suffix follows them. In JSON, `links` becomes `{outgoing, incoming, usage, left_out, omitted}`, and `usage` is `{used_by, mentioned_by}`. Both are tallied from the In rows with `tally`, live rows only, so `--archive` widens In and never the counts. MCP `get_node(with: [links])` on stdio and on a hub's `/mcp` serves the same, at plugin 0.46.0.

### `spec catalog`

A row's text gains one field after `tasks`:

```text
MEC-STAMINA | version 1 | no card | open items 0 | tasks 0, open 0 | used by 2, mentioned by 6 | - | - | - | Stamina
A-102 | version 1 | no card | open items 0 | tasks 0, open 0 | used by 0, mentioned by 0, orphan | - | - | - | The lantern burns one unit of oil per minute
```

The JSON row gains `used_by`, `mentioned_by` and `orphan`, for 16 keys. The counts come from the graph the rows' tree read built, in one more pass over its edges.

### `spec health`

Row 8 is `orphans`. JSON gains `orphans: {live, rows, notes}`:
- `live` is the number of live documents judged;
- a row is `{name, id, kind, title, path, line, status, tasks, tasks_open}`, its tasks those of the trace's `node_work` (cancelled ones ignored), or `null` both when the queue was not read;
- rows are sorted by `kind` in code-point order (`null` last), then by path;
- `notes`: `orphans: no queue yet: no task read`, or `orphans: tasks not read: <cause>`.

The text adds a block after rows 4-6's lines. The last line is unchanged:

```text
orphans: 1 of 12 live documents link to nothing
  A-102 | requirement | The lantern burns one unit of oil per minute | docs/records/A/A-102.md:1
spec health: 13 documents, 0 unapplied amendments
```

A row's tail is ` | tasks <t>, open <o>` when it holds a task. The block is uncut. The input is `spec check`'s: `SpecGraph::new` now runs on every call, not only when an amendment is listed.

### Daemon, keys, plugin

The catalog, health and nodes routes pass the JSON through. No route, parameter or MCP tool is added. Key sets in `fixtures/daemon-keys.json`:
- `CatalogRow` goes from 13 to 16 keys;
- the links set gains `usage`, and the health set gains `orphans`;
- new sets: `LinkUsage` (2), `HealthOrphans` (3), `HealthOrphan` (9).

`plugin.json` goes to 0.46.0, with a new `PINS` entry. No description, skill or role text changes: mcp `read.rs`' `WITH_DOC` names no key of the links.

### The UI

- **Types** (`provisional.ts`, each citing this spec's criteria until canon exists): `LinkUsage`, `ShownLinks.usage`, `CatalogRow.{used_by, mentioned_by, orphan}`, `HealthOrphans`, `HealthOrphan`, `HealthReport.orphans`. Mocks (harbor-sim) carry a used document, an unused one, a section and two orphans, one of them with an open task. `empty` has none. `large` has 3 000 orphans.
- **Links** (`doc/DocLinks.tsx`, grouping in `doc/usage.ts`, new):
  - Under In comes the served line `doc.links.usage` ("Used by {{used}} · Mentioned by {{mentioned}}", plural).
  - When `used_by` is 0, `doc.links.unused` takes the groups' place.
  - Otherwise come the strong resolved In rows whose `path` is not the holder's, grouped by `end_kind` as `insideOf` groups children: direction sets first, in served order (`insideDirections` over the held workspaces read), then raw kinds in code-point order (`lang="en"`), `null` last (`doc.inside.noKind`).
  - Each group is a `role="group"` labelled "<label> · <n>", where `n` counts its distinct `name`s.
  - A user is a `DocLink` (title, else name) followed by its places: `path:line` in mono, plus ` at <section>` when `at` is set.
  - A group shows 12 users (`USED_BY_SHOWN`), then "Show all <n>", which moves focus to the 13th.
  - Rows written in the holder's own file form a last group, "Within this document" (`doc.links.sameDocument`), outside the count.
  - Mentions and Out are unchanged.
- **Catalog** (`tree/DirectionCatalog.tsx`, `tree/catalog.ts`):
  - A "Used by" column after Tasks shows `used_by`, then "+ M mentions" in small text when `mentioned_by` > 0. An orphan gets a neutral `Badge` "Orphan". The cell never wraps.
  - Sorting is descending first; ties keep the served order.
  - A `Button` "Used by 0" (`aria-pressed`) beside the caption keeps the rows whose `used_by` is 0. The caption then reads "<title>: K of N nodes". With no such row, the table shows an `EmptyState`.
- **Health** (`health/OrphansPart.tsx`, new, mounted in `LeftRegion.tsx` after Drift, before the Budgets pointer):
  - It reads `getHealth` and keeps row 3's three states: being measured, could not be measured, measured.
  - Measured: "<n> orphan documents over <live> live documents", with one line of meaning: "Linked to nothing: no link to or from another document and no child. Link it in, or set it aside through a proposal."
  - Rows are grouped by direction, then kind, as in Links.
  - A row is a `DocLink`, its `path:line`, its raw status (neutral, `lang="en"`) and its tasks ("No task", or "N · M open").
  - It shows 10 rows (`SHOWN_OPEN`), then "Show all <n>". The orphans part's `notes` are shown `lang="en"`.
  - Zero: "None: every live document links or is linked.", then "Measured over <live> live documents."
- **Strings.** About 22 leaves: `doc.links.*`, `tree.catalog.usedBy*`, `health.orphans.*`, in `en.ts` and `ru.ts`, the Russian a draft, with plurals. **CSS**: `.doc-usage-*`, `.catalog-used-*`, `.health-orphans-*` in `app.css`, from tokens only.

## Rules and edge cases

- WHEN a source cites a node several times, or cites it and its section, it SHALL count once. WHEN it sits in the node's own file, it SHALL not count.
- WHEN a link is `dangling`, `skipped` (`project:`) or `unchecked`, it SHALL count neither as a use nor as a link out. A document whose only links go to another project or to code files is an orphan (known limit).
- WHEN `--archive` is given, In rows MAY include Tier 3 rows, and the counts SHALL stay those of the live files.
- WHEN a document's front-matter fails, it is live (`is_live`) and judged by its parse.
- A wiki-style `[[Title]]` is no link, so a document cited only that way shows as an orphan. An ID or a path in the brackets is a link.
- One corpus, queue and config SHALL give byte-identical text and JSON at any root and `HOME`, blind to input order.
- Health's orphan row SHALL never change its exit, and neither SHALL a failed queue read.

## Acceptance criteria

A browser look is marked **[look]** (the orchestrator's, in Chrome), and a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turns red. Code comments cite `docs/features/usage-orphans.md` "Acceptance criteria" with an ID, never "Data" (`doc_pointers`). "The copy" is a scratch copy of `fixtures/spec-a`.

- [ ] UO1 — **The counts.** On the copy:
  - `spec show <REF> --links --json` gives `links.usage` as Data's table for every node in it;
  - `spec catalog --kind mechanic --json` gives MEC-SPRINT `{1, 1, false}` and MEC-STAMINA `{2, 6, false}`, and `--kind rule` gives RULE-STAM-REGEN `{2, 3, null}`;
  - core `usage_of` over the whole graph equals `show`'s usage for every node of `spec-a` and `spec-b`.

  Mutations, each red: rows counted, not sources (MEC-STAMINA and R-12 `mentioned_by` 7); same-file sources counted (`stamina-tuning.md` 2); a document without its sections' uses (MEC-STAMINA `used_by` 1).
- [ ] UO2 — **Live sources only.** DEC-0023 keeps `used_by` 1, with `--archive` too, while its In list gains DEC-0007's rows. A scratch `class: generated` file whose `links:` cites A-102 leaves A-102 at 0 and an orphan. Mutations, each red: Tier 3 rows counted under `--archive`; a generated source counted.
- [ ] UO3 — **The orphan rules.** The copy gains four files:
  - `docs/spec/movement/lantern.md` (`id: MEC-LANTERN`, `parent: DOM-MOVEMENT`, no link) is an orphan;
  - `docs/spec/lone.md` (`id: DOM-LONE`, no link) is not, because `docs/spec/lone-child.md` (`id: MEC-LONE-CHILD`, `parent: DOM-LONE`) is its child; that child is an orphan;
  - `docs/spec/old.md` (`id: MEC-OLD`, `status: abandoned`, no link) is not.

  A-102 is an orphan and Q-032 is not. Mutations, each red: one's own `parent:` taken as a link; children ignored; `abandoned` ignored; links out ignored.
- [ ] UO4 — **Health's row.** On the copy, `spec health --json` gives `orphans` `{live: 12, rows: [A-102's], notes: []}`, and the text is Data's three lines. The CLI then makes a card on A-102 (`spec card new … --target A-102`). The row stays and reads `tasks 1, open 1`. With the queue at a newer schema, the rows read `tasks: null` with the note, and the exit is 0. `GET …/health` is byte-equal to the CLI's JSON. Mutations, each red: a document with an open card left out; Tier 3 documents judged (`live` 13); a queue error failing the read.
- [ ] UO5 — **The catalog row.** Data's two text rows, a section row with no `orphan` word, and the JSON's 16 keys. `GET …/catalog` stays byte-equal, and the scene's pins in `tests/catalog_scene/` are updated with the new field alone. Mutation, red: `orphan` set on a section.
- [ ] UO6 — **`show --links` and `get_node`.** MEC-STAMINA's block ends `links 5 out, 10 in; used by 2, mentioned by 6`. `get_node` with `links` on stdio MCP and on a hub's `/mcp` serves `usage` equal to the CLI's. Plain `show` serves `links: null`, as before. `plugin.json` is 0.46.0, and `plugin_files.rs` is green with the new `PINS` entry. Mutations, each red: the counts tallied over admitted rows (they change under `--archive`); the summary line without the counts.
- [ ] UO7 — **Nothing stored.** After warm `catalog`, `health` and `show --links` reads, the index database's `sqlite_master` and each table's row count equal those before the reads. `CHECK_CODES`, the queue schema, the backup format and `INDEX_FORMAT` are unchanged. Mutation, red: a cache table of counts.
- [ ] UO8 — **No domain, one answer.** `usage.rs` holds no string literal but `"abandoned"`, checked by a source scan in its test. http `no_domain.rs` and eval `workspace_names.rs` pass. The copy at two roots and two `HOME`s, its files written in opposite orders, gives byte-identical `catalog`, `health` and `show --links` text and JSON. Mutation, red: rows taken in input order.
- [ ] UO9 — **Keys.** `fixtures/daemon-keys.json` holds the sets from Data, written once by `SPECENGINE_WRITE_DAEMON_KEYS=1`; `daemonKeys.test.ts` is green, and `AHEAD_OF_FIXTURE` is empty at the end. Mutation, red: a set left out.
- [ ] UO10 — **The document view** (`doc/usage.test.ts`, `DocLinks.test.tsx`, on the mock). MEC-STAMINA shows "Used by 2 · Mentioned by 6" and groups whose counts sum to the served `used_by`. Each user lists every place (`docs/spec/movement/sprint.md:9`, `:10 at RULE-STAM-REGEN`). An unused document shows `doc.links.unused` with no group. A same-file row sits under "Within this document". The 13th user of a group shows only after "Show all". Mutations, each red: a group counting rows, not names; the zero line keyed on the rows, not the served count; a same-file row counted in its kind's group.
- [ ] UO11 — **The catalog column and Health's part** (`DirectionCatalog.test.tsx`, `catalog.test.ts`, `HealthView.test.tsx`, the mocks' smoke tests):
  - The column shows the count, the mention line and the Orphan badge, and its sort puts the highest first.
  - "Used by 0" keeps the unused rows and changes the caption.
  - Health's part shows its three states, groups by direction, the task line, Show all past 10 and the zero lines.
  - `policy.test.ts` and `words.test.ts` pass.

  Mutations, each red: the filter on `mentioned_by`; the part reading 0 while it is still being measured.
- [ ] UO12 — **[measure] Cost.** Eval `usage_scale.rs` (one default check, its measures `#[ignore]`), on `catalog_scale.rs`'s generated tree, warm, release, median of 5:
  - at 10k nodes the catalog's whole read stays within its 2 s bar, and the usage pass adds at most 100 ms;
  - `spec health --json` takes at most 1.5 s at 10k;
  - the 1k and 100k figures are recorded.

  On a read-only copy of pilot A, the orchestrator records the live documents, the orphans and Health's time in "Implementation", never naming the project. Past a quarter of live documents as orphans, the figure goes to the owner before ship (the noise risk).
- [ ] UO13 — **[look]** In Chrome, both themes and both languages, at 1440x900 and 1100x800: a document's Links with groups and places; an unused document; the catalog with the column, the badge and the filter; Health's part with an orphan holding a task. Keyboard only, Tab reaches "Show all", the filter and each row's link.
- [ ] UO14 — **Gates.** These pass:
  - `cargo nextest run` for `specengine-core`, `-cli`, `-mcp` and `-http`;
  - eval `doc_pointers`, `ui_policy`, `anonymity` and `workspace_names`;
  - UI lint, `tsc`, `pnpm test` and `pnpm build`, with first-load JS at most 1.5 kB over the figure before this slice and within 845 kB, the figure recorded;
  - `export index`, then `check`.

## Out of scope

- An orphan check finding, a gate or any block (ADR-0012).
- A stored state for "bring in later" (an open card is that), and a delete act: `status: abandoned` goes through a proposal.
- Coverage findings, such as "every design document is owned by a mechanic" (the prototype's idea, a later item).
- `[[Title]]` as a link.
- Counts through `parent:`.
- `orphan` in `show`.
- Flow's `definition-unused`, which stays as it is: two notions, each named in its canon.
- Usage in `spec tree` or `search`.
- The map (`corpus-map`, RM-0112).

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | core `src/usage.rs` (new), `src/lib.rs` (`pub mod usage;`); CLI `src/links.rs` (`ShownLinks.usage`, the summary), `src/catalog.rs` (row keys, text), `src/health.rs` (row 8, its queue read through `trace.rs` `node_work`), `src/tree.rs` only if `direction_lines` must hand its graph to the catalog; `plugin/specengine/.claude-plugin/plugin.json` 0.46.0 |
| test-engineer, Rust | core `tests/usage.rs`; CLI `tests/{links,catalog,health}.rs`, `tests/catalog_scene/` pins, a `usage_orphans.rs` for UO1-UO4 and UO7-UO8; mcp and http tests (UO6, UO9); eval `tests/usage_scale.rs`; `plugin_files.rs` `PINS`; `daemon_keys.rs` with `fixtures/daemon-keys.json` and `daemonKeys.test.ts` in one change |
| ui-developer | `api/provisional.ts`, `test/builders.ts`, the mocks (catalog, health, node), `doc/{DocLinks.tsx,usage.ts}`, `tree/{DirectionCatalog.tsx,catalog.ts}`, `health/{OrphansPart.tsx,LeftRegion.tsx}`, `i18n/{en,ru}.ts`, `styles/app.css` |
| test-engineer, UI | `doc/usage.test.ts`, `DocLinks.test.tsx`, `DirectionCatalog.test.tsx`, `catalog.test.ts`, `HealthView.test.tsx`, the smoke tests; UO10, UO11 and UO14's mutations and figure |

### Overlaps

- **`delivery-timeline`** (in flight). This slice touches none of its Rust files (CLI `src/{lib,main,subset,delivery}.rs`, core `src/delivery.rs`, http `src/app.rs`). If `catalog.rs` needs a `subset.rs` seam, that step waits for its commit. In the UI both edit `provisional.ts`, `test/builders.ts`, `en.ts`, `ru.ts` and `app.css`. The fixture `daemon-keys.json` is regenerated once per lane, one lane at a time.
- **`release-hardening`** (in flight). Its lane U edits `health/LeftRegion.tsx` (RH-U4); the one mount line here lands after it. Its D11 and RH-R7 leave the plugin to this slice's 0.46.0. No Rust file is shared.
- **`corpus-map`** reads `usage.rs` and edits core `src/lib.rs`, `provisional.ts`, `builders.ts`, the catalogs, `app.css` and the keys fixture. **`scope-rings`** edits core `src/lib.rs` and the same UI files.

### Work order

1. Core `usage.rs` with its tests, then CLI `links.rs`, `catalog.rs` and `health.rs`, one Rust lane, now: no in-flight file is touched. Then the plugin bump.
2. The UI behind `AHEAD_OF_FIXTURE`, once `delivery-timeline`'s UI has committed its shared files. The `LeftRegion.tsx` mount comes after RH-U4.
3. Tests overlap each step. Then the reviews, the look (UO13) and the measures (UO12). At most three iterations a lane.
4. Ship ("Canon at ship"); `corpus-map`'s core may then start on `usage_of`.

### Canon at ship

| File | Edit |
|---|---|
| `docs/canon/spec-usage.md` (new, Tier 2) | the rule (D2-D4), the counts' homes (show, catalog, health, map), JSON and text, the spec-a table's first rows, known limits (`[[Title]]`, other-project links, Flow's other notion), cost, tests |
| `spec-cli-graph.md` "spec show --links" | `usage` and the summary's counts, pointing at `spec-usage.md`. Net ≤ 0 (16 B free) |
| `spec-catalog.md` "Text and JSON", "Daemon, hub" | the field and keys; key set 16. Net ≤ +300 B (916 B free) |
| `health.md` intro, "spec health", "UI" | row 8 a pointer to `spec-usage.md`. Net ≤ 0 (13 B free) |
| `doc-view.md` "The view" | the In line and groups, pointing at `spec-usage.md`. Net ≤ 0 |
| `directions.md` "Tree" | the column and the filter. Net ≤ +200 B |
| `plugin-files.md` "Version" | 0.46.0, `get_node`'s links `usage` |
| `ui-languages.md` "Size" | the figure. Net |
| `CLAUDE.md` "State", 08 Phase 7, RM-0111 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers

- **The owner's:** Q1 (an orphan is linked to nothing) and a "used by 0" filter.
- **The analyst's,** taken: Q2 (no stored state: an open card, or `abandoned` by a proposal), sources from other files only, `parent:` not a use, live sources only.
- **The spec-writer's:**
  - D6, served to MCP at plugin 0.46.0, because the UI must not count;
  - D7, no orphan in `show`, so `subset.rs` is left alone;
  - D8, Health judges every live document whatever `[paths] spec` says, groups in the UI, and reads tasks on a best-effort basis;
  - the rows of the holder's own file grouped apart in Links, so the groups sum to the served count;
  - 12 users a group, after the prototype's first 12 names.
