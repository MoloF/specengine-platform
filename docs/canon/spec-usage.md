---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-13
---

# spec usage: what uses a document, and which documents nothing links

Built by `usage-orphans` (RM-0111), no ADR: it rests on ADR-0008 (the core knows no domain), ADR-0012 (nothing blocks) and the convention's `abandoned` (`documentation-system.md` §6). Two counts and one flag, computed per read from the resolved graph, never stored: no table, column, `INDEX_FORMAT`, queue schema, backup format, `CHECK_CODES` entry or config key. An orphan is no check finding and blocks nothing. Code: core `usage.rs` (`NodeUsage`, `UseRow`, `tally`, `usage_of`); CLI `links.rs` (`LinkUsage`, `ShownLinks.usage`), `catalog.rs`, `health.rs` (row 8), `trace.rs` (`node_work`, `QueueRead::is_unborn`); MCP `mirror.rs` (`ShowLinks.usage`). Criteria: `docs/features/usage-orphans.md`.

## The rule

- **A use of X** is a link that is `resolved`, ends on X or on a node nested in X (a document: any of its ID sections), and is written in a live file (`is_live`: neither Tier 3 nor `class: generated`) other than X's, whatever `--archive` admits. Its **source** is the innermost ID section around the link, else the document, else the file's path (`spec-cli-graph.md` "Parents and links"); for `status: superseded-by D` the source is D.
- `used_by` counts distinct sources over the strong types, `mentioned_by` over `mentions` (`is_weak_link`); a source may count in both, and once however often it cites the node or its sections. `parent:` is containment, never a use.
- **An orphan** is a live document linked to nothing, all of these holding: its `status:` is not `abandoned`; both counts are 0; no resolved link written in its file ends on a node of another file, whatever that file's standing; no link of another live file has it as its source (a live `superseded-by` it links it); no live document of another file has a resolved `parent:` ending on it or one of its sections. Its own `parent:` is no link; a `parent:` cycle keeps a child. A section, and every node of a Tier 3 or generated file, keeps its counts and is not judged: `orphan` `null`.
- `dangling`, `skipped` (`project:`) and `unchecked` links count neither as a use nor as a link out. A wiki-style `[[Title]]` is no link; an ID or a path in the brackets is.
- An open card on an orphan means "bring it in later": the row stays and shows its tasks. Setting it aside is a proposal setting `status: abandoned`. Neither is stored state.
- The core reads no kind, link type or class name, only the model's weak type and `abandoned`. One graph gives one answer whatever its files' order: nodes and edges in (path, position) order, sources counted as sets.

On `fixtures/spec-a` (12 live documents; DEC-0007 Tier 3), pinned in CLI `tests/usage_orphans.rs`: MEC-STAMINA used by 2, mentioned by 6 (DEC-0023 and MEC-SPRINT, the two through RULE-STAM-REGEN; Q-031 mentions it twice, counted once); RULE-STAM-REGEN 2 and 3, a section; DEC-0023 1 and 1 (DEC-0007's `supersedes` is Tier 3); `docs/features/stamina-tuning.md` 0 and 1 (its own AC-07's mention is the same file); A-102 0 and 0, the one orphan; Q-032 0 and 0, no orphan (it links out through `working_answer`); DOM-GAME no orphan (links out, a child).

## Where it is served

- **`spec show REF --links`** (`spec-cli-graph.md` "spec show --links"): the block's last line `links <o> out, <i> in; used by <u>, mentioned by <m>[; left out: …]`; JSON `links.usage` `{used_by, mentioned_by}`, tallied by `tally` over the In rows the read already holds (`spec-read-subset.md` "The subset": every row ending on the node or its sections), live rows only, so `--archive` widens In, never the counts. No `orphan` there: it needs children, a whole-graph read. MCP `get_node` with `links`, on stdio and on a hub's `/mcp`, serves the same (plugin 0.46.0).
- **`spec catalog`** (`spec-catalog.md` "Text and JSON"): a row's `used_by`, `mentioned_by`, `orphan`, from `usage_of` over the graph the rows' tree read built, one more pass over its edges.
- **`spec health`** row 8 (below). **`corpus-map`** (RM-0112) reuses the counts and the flag.

## Health

`spec health` judges every live document of its walk, wherever it sits, `[paths] spec` or not: `SpecGraph::new` over `spec check`'s input runs on every call. JSON `orphans: {live, rows, notes}`, its last key: `live` the live documents judged; a row `{name, id, kind, title, path, line, status, tasks, tasks_open}`, by `kind` in code-point order (`null` last), then by path. Tasks are the trace's `node_work` count of the row's cards, cancelled ones ignored, read from the queue only when there is a row, without the index; a queue that is not there or does not read leaves both `null` with one note, never a failure or another exit. `notes`, only with a row, sorted, each once: `orphans: no queue yet: no task read`, `orphans: tasks not read: <cause>`, or the card read's own notes `orphans: `-prefixed; each is also a stderr `note:` line, escaped, as `spec catalog` prints its own.

Text, after rows 4-6's lines, uncut; a row's tail ` | tasks <t>, open <o>` when it holds a task:

```text
orphans: 1 of 12 live documents link to nothing
  A-102 | requirement | The lantern burns one unit of oil per minute | docs/records/A/A-102.md:1
spec health: 13 documents, 0 unapplied amendments
```

One corpus, queue and config give byte-identical text and JSON at any root and `HOME`, whatever the files' order.

## Daemon and keys

The nodes, catalog and health routes pass the JSON through; no route, parameter or MCP tool is added. `fixtures/daemon-keys.json`: `CatalogRow` 16 keys, the links set with `usage`, the health set with `orphans`, and `LinkUsage` (2), `HealthOrphans` (3), `HealthOrphan` (9), sets 149 to 151 of 151.

## UI

The UI counts nothing (`architecture.md#ui`); it groups the served In rows it already lists.

- **Links** (`doc/DocLinks.tsx`, `doc/usage.ts`): under In, `doc.links.usage` "Used by N · Mentioned by M"; at `used_by` 0, "Declared, nothing uses it yet." in place of the groups. Else the strong resolved In rows of other files, grouped by `end_kind` as Inside groups children (direction sets first, in served order, then raw kinds, `null` last), each a `role="group"` "<label> · <n>", `n` its distinct names; a user a `DocLink`, then each place: `path:line`, its raw type (neutral, `lang="en"`), ` at <section>` when set. 12 users a group (`USED_BY_SHOWN`), then "Show all <n>", focus on the 13th. The holder's own file's rows form a last group, "Within this document", outside the count. Mentions and Out are unchanged.
- **Catalog**: the Used by column and its two filters, `directions.md` "Tree".
- **Health** (`health/OrphansPart.tsx`, after Drift in `LeftRegion.tsx`): row 3's three states; measured "<n> orphan documents over <live> live documents" and one line of meaning; rows grouped by direction, then kind; a row a `DocLink`, `path:line`, its raw status, its tasks ("No task", else "N · M open"); 10 rows (`SHOWN_OPEN`), then "Show all"; zero "None: every live document links or is linked." then "Measured over <live> live documents."; the notes `lang="en"`.

## Known limits

- Sources dedup by name, so a duplicated ID counts once.
- A `class: spec` document `abandoned` is Tier 3, never judged.
- A document whose only links go to another project or to code files is an orphan.
- Flow's `definition-unused` (`uses` alone, on `[[flow.definition]]` documents) is another notion and stays.
- When the links read cuts rows (`omitted`), the Links groups can sum to less than `used_by`, unsaid.
- The catalog's rows are `spec tree --kind`'s, so a record outside `[paths] spec` has no row there; Health judges it.
- "Used by 0" reads as dead on records, cited mostly through `mentions`: the catalog's 0 shows its mentions at full weight, and "Not cited" keeps the rows with both counts 0 (the orchestrator's working answer for the owner).
- **Cost** (`eval/tests/usage_scale.rs` on `catalog_scale.rs`'s tree, release, warm, median of 5, a loaded machine): the usage pass 0, 5 and 80 ms at 1k, 10k and 100k nodes; `spec catalog --kind node --json` 253 ms, 1.37 s and 2.55 s (bar 2 s at 10k); `spec health --json` 17 ms, 165 ms and 1.70 s (bar 1.5 s at 10k). A scratch copy of a real game corpus: 18 orphans of 1 397 live documents (1.3 %: assumptions 4, questions 4, no kind 4, decisions 2, plan items 2, spikes 2); Health 2.2-2.8 s on a debug build, its release time owed.

## Tests

Core `tests/usage.rs` (a source scan: no literal but `"abandoned"`); CLI `tests/usage_orphans.rs`, `links.rs`, `catalog.rs` (`catalog_scene/`), `health.rs`, `health_rows/`; MCP and http `usage_orphans.rs`; http `daemon_keys.rs`; eval `usage_scale.rs`; MCP `plugin_files.rs`. UI `doc/{usage,DocLinks}.test.*`, `tree/{catalog,DirectionCatalog}.test.*`, `health/HealthView.test.tsx`, `mocks/{orphans,usage.smoke}.test.*`, `api/daemonKeys.test.ts`.
