---
class: spec
status: shipped
shipped: 2026-11-13
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, plugin, ui]
ref: RM-0111; the owner's goal of 2026-11-13 (writers see what is used where, by which documents and in which places, and orphan documents show up); the analyst's analysis of 2026-11-13, slice 1, with the owner's answers (Q1, an orphan is linked to nothing; Q2 a working answer); an earlier prototype's "filled by" panel
adrs: []
---

# usage-orphans: what uses a document, where, and which documents nothing links

## Why

Writers describe documents and reuse props and mechanics through links, but could not see what uses a document without reading every In row, no list carried a usage count, and nothing named the documents linked to nothing, the ones to delete or bring in later. This slice serves one count per read, never stored, from core `usage.rs`: the document view's Links group a document's users by kind with each place, the direction catalog gets a "Used by" column and two filters, Health gets row 8, `orphans`, over the whole corpus, and `get_node` serves the numbers to agents (plugin 0.46.0). An orphan is linked to nothing (the owner's Q1); an open card means "bring it in later" and `status: abandoned` "set aside" (Q2, a working answer). No ADR. Canon: `docs/canon/spec-usage.md`.

## Rules and edge cases

The rule, its edge cases and the determinism clause are canon: `docs/canon/spec-usage.md` "The rule", "Health", "Known limits". In short: a source counts once however often it cites a node or its sections, never from the node's own file; `dangling`, `skipped` and `unchecked` links count neither way; `--archive` widens In, never the counts; a front-matter failure is still live and judged by its parse; one corpus, queue and config give byte-identical text and JSON at any root and `HOME`, blind to input order; Health's orphan row and a failed queue read never change its exit.

## Acceptance criteria

**[look]** marks a browser look, **[measure]** a timed or counted run; every other criterion is a test its named mutation turned red. Code comments cite this heading with an ID (`doc_pointers`). "The copy" is a scratch copy of `fixtures/spec-a`. Evidence: the test-engineer's tests citing each ID, and the Rust (iteration 1, then the stderr ruling) and UI reviews, ACCEPT.

- [x] UO1 — **The counts.** On the copy `show --links --json` gives each node's `links.usage` as pinned in CLI `tests/usage_orphans.rs` (MEC-STAMINA `{2, 6}`, R-12 `{1, 6}`, `stamina-tuning.md` `{0, 1}`); `catalog --kind mechanic` MEC-SPRINT `{1, 1, false}`, MEC-STAMINA `{2, 6, false}`, `--kind rule` RULE-STAM-REGEN `{2, 3, null}`; core `usage_of` equals `show`'s usage on every node of `spec-a` and `spec-b`. Mutations red: rows counted, not sources; same-file sources counted; a document without its sections' uses.
- [x] UO2 — **Live sources only.** DEC-0023 keeps `used_by` 1 with `--archive` while its In gains DEC-0007's rows; a `class: generated` file citing A-102 leaves it 0 and an orphan. Mutations red: Tier 3 rows counted; a generated source counted.
- [x] UO3 — **The orphan rules.** `docs/spec/movement/lantern.md` (MEC-LANTERN, `parent: DOM-MOVEMENT`) is an orphan; DOM-LONE is not, its child MEC-LONE-CHILD is; MEC-OLD (`abandoned`) is not; A-102 is, Q-032 is not. Mutations red: one's own `parent:` taken as a link; children ignored; `abandoned` ignored; links out ignored.
- [x] UO4 — **Health's row.** With a queue (no queue gives the note `orphans: no queue yet: no task read`), `spec health --json` gives `orphans` `{live: 12, rows: [A-102's], notes: []}` and the text `spec-usage.md` "Health" shows; a card on A-102 keeps the row, `tasks 1, open 1`; a queue at a newer schema gives `tasks: null`, the note, exit 0; `GET …/health` byte-equal. Mutations red: a document with an open card left out; Tier 3 judged (`live` 13); a queue error failing the read.
- [x] UO5 — **The catalog row.** With `[paths] spec = "docs"` (so A-102's record has a row), the text rows `… | used by 2, mentioned by 6 | …` and `… | used by 0, mentioned by 0, orphan | …`, a section row without `orphan`, 16 JSON keys; `GET …/catalog` byte-equal; the scene's pins changed by the new field alone. Mutation red: `orphan` on a section.
- [x] UO6 — **`show --links` and `get_node`.** MEC-STAMINA's block ends `links 5 out, 10 in; used by 2, mentioned by 6`; stdio and hub `get_node` serve `usage` equal to the CLI's; plain `show` `links: null`; `plugin.json` 0.46.0, `PINS` `a18d9890…d694`, `SCHEMA_PINS` `9aba4ca4…f0f3`. Mutations red: the counts over admitted rows; the summary without them.
- [x] UO7 — **Nothing stored.** Warm `catalog`, `health`, `show --links` leave `sqlite_master` and every table's row count unchanged; `CHECK_CODES`, the queue schema, the backup format and `INDEX_FORMAT` unchanged. Mutation red: a cache table.
- [x] UO8 — **No domain, one answer.** `usage.rs` holds no literal but `"abandoned"` (a source scan); http `no_domain.rs`, eval `workspace_names.rs` pass; two roots and two `HOME`s, files written in opposite orders, give byte-identical text and JSON. Mutation red: rows in input order.
- [x] UO9 — **Keys.** `fixtures/daemon-keys.json` gains `LinkUsage`, `HealthOrphans`, `HealthOrphan` (sets 149-151), `CatalogRow` 16, the links set `usage`, the health set `orphans`; `daemonKeys.test.ts` green, `AHEAD_OF_FIXTURE` empty. Mutation red: a set left out.
- [x] UO10 — **The document view** (`doc/usage.test.ts`, `DocLinks.test.tsx`): "Used by 2 · Mentioned by 6", groups summing to `used_by`, every place (`sprint.md:9`, `:10 at RULE-STAM-REGEN`), the unused line, "Within this document", the 13th user after "Show all". Mutations red: a group counting rows; the zero line keyed on rows; a same-file row in its kind's group.
- [x] UO11 — **The catalog column and Health's part** (`DirectionCatalog.test.tsx`, `catalog.test.ts`, `HealthView.test.tsx`, smoke tests): count, mentions and badge, the sort; the filters and the caption; Health's three states, groups, task line, Show all past 10, zero lines; `policy.test.ts`, `words.test.ts`. Mutations red: the filter on `mentioned_by`; 0 while measuring.
- [x] UO12 — **[measure] Cost.** Eval `usage_scale.rs`, release, warm, median of 5, a loaded machine: at 10k the catalog 1 373 ms (bar 2 s), the usage pass 5 ms (bar 100 ms), `health --json` 165 ms (bar 1.5 s); 1k 253, 0, 17 ms; 100k 2 553, 80, 1 696 ms. A scratch copy of a real game corpus: 18 orphans of 1 397 live documents (1.3 %, under the quarter that would go to the owner); Health 2.2-2.8 s on a debug build, the release figure owed.
- [ ] UO13 — **[look]**, partly: Russian, light at 1440 (Links' groups and places, the catalog's column, badges and filter group) and dark at 1100 (Health's part, a row with a task). **Owed:** English in the browser, and the keyboard pass (Show all, the filter, each row's link). At 1440 the table scrolls sideways for its last column, as before.
- [ ] UO14 — **Gates.** At ship: `export index` and `check` clean; eval `doc_pointers`, `ui_policy`, `anonymity` pass; first load 835.71 kB (+1.29 kB, within 845); UI lint, `tsc`, `pnpm test`, `pnpm build` at the UI review. The Rust run under load had this slice's two pins (`kind_amendment` 151, `.env.example` 0.46.0), since fixed. The clean run is `release-hardening`'s RH-G1, on the joint commit.

## Out of scope

An orphan finding, gate or block (ADR-0012); a stored "bring in later" state or a delete act; coverage findings; `[[Title]]` as a link; counts through `parent:`; `orphan` in `show`; Flow's `definition-unused`, unchanged; usage in `spec tree` or `search`; the map (`corpus-map`, RM-0112).

## Implementation

| Module | What it does |
|---|---|
| core `usage.rs`, `lib.rs` | `NodeUsage`, `UseRow`, `tally`, `usage_of`; the only literal `"abandoned"` |
| CLI `links.rs` | `LinkUsage`, `ShownLinks.usage`, the summary's counts |
| CLI `catalog.rs` | `used_by`, `mentioned_by`, `orphan`; 16 keys; the text field |
| CLI `health.rs`, `trace.rs` | row 8 `orphans {live, rows, notes}`, the graph on every call, the tasks by `node_work`; `QueueRead::is_unborn` |
| MCP `mirror.rs`; `plugin.json`, `scripts/deploy/.env.example` | `ShowLinks.usage`; 0.46.0 |
| UI `api/provisional.ts`, `doc/{DocLinks.tsx,usage.ts,inside.ts}`, `tree/{DirectionCatalog.tsx,catalog.ts}`, `health/{OrphansPart,LeftRegion}.tsx`, `mocks/{orphans,catalog,health,corpus,scenario}.ts`, `i18n/`, `styles/` | types; Links' groups; the column and two filters; Health's part; mocks; strings; CSS |
| tests | core `usage.rs`; CLI `usage_orphans.rs` and the pins of `links`, `catalog`, `health`, `show`, `bounds`, `read_trust`, `store_reads`; MCP, http `usage_orphans.rs`; `daemon_keys.rs`, `plugin_files.rs`, `mcp_size.rs`; eval `usage_scale.rs`; UI tests and stubs |

**Rust departures**, accepted: `usage_of` covers every file's nodes, Tier 3 and generated ones with counts and `orphan: None`; a live `superseded-by D` links D; a `parent:` cycle keeps a child; Health reads tasks without the index.

**Rulings (orchestrator):** row 8's notes and the card read's go to stderr as `note:` lines, as the catalog's (reversing "no stderr", iteration 2). "Used by 0" reads as dead on records, cited mostly by `mentions`, so the column shows "0 · N mentions" at normal weight and the one "Used by 0" filter became two, "No declared use" and "Not cited" (a working answer for the owner). Links keep each place's raw link type (UI review).

**Known limits:** a duplicated ID counts once; an `abandoned` spec is Tier 3, unjudged; Links' groups can sum below `used_by` when the read cuts rows, unsaid; a filter change keeps the "Show more" count; the Russian is a draft.
