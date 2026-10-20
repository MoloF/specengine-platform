---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli]
owner: owner
reviewed: 2026-10-20
---

# spec roadmap: plan items and the computed path

Plan items are `spec`-class files; the path and the pool are computed per read from the files, never stored; the chain orders work, never gates it (ADR-0052, `architecture.md#storage`, ADR-0012). The vocabulary is `[roadmap]`'s alone (ADR-0008).

## Config

```toml
[roadmap]
prefix     = "RM"                         # required: an [ids] prefix, shape number, scope project
dir        = "docs/roadmap"               # required: where items are created; the [paths] path rules
releases   = ["mvp", "v1", "v2"]          # required: the path's order
mvp_cut    = "mvp"                        # optional: one of releases
priorities = ["must", "should", "could"]  # required: highest first
unit       = "days"                       # optional: printed after every sum, never converted
```

Core `roadmap_toml.rs`: `RoadmapConfig::from_toml(text, &IdScheme) -> Result<Option<_>, ProjectError>`, `None` without the table; the six fields plus `scheme` (the `[ids]` judged against) and `paths` (the text's `[paths]`), so `plan` resolves `depends_on` as `spec check` does. `ProjectConfig` only name-checks `roadmap`: a bad table fails `spec roadmap` alone. Every value is judged; the first error by line prints `<config>:<line>: <message>`, exit 2: another key (`unknown field`); a required key missing (`` `[roadmap]` has no `dir`: it is required ``, at the header line); a wrong type (`` `roadmap.prefix`: expected a string, found an integer ``); `prefix` not in `[ids]`, an alias, of shape `name` or feature-scoped; `dir` failing `checked_path`; `releases` or `priorities` empty, an entry not a slug (`slug_problem`: `grammar::is_slug`, then at most 64 bytes) or repeated (`` `roadmap.releases`: "v1" is repeated ``); `mvp_cut` not in `releases` (judged once they read); `unit` blank, over 32 bytes or holding a control character.

Here: `RM` (`plan-item`, width 4) in `docs/roadmap/`, shard `docs/index-roadmap.md`; `mvp` the rest of Phases 0-2, `v1` Phases 3, 4, 6, `v2` Phase 5. An older `spec`, `specengine-mcp` or `specengine-http` refuses a config holding `[roadmap]` and an `INDEX_FORMAT` 12 index: install the three from one commit.

## Items

A document, live or Tier 3, whose front-matter `id:` has the prefix as written (not an alias); never a `{#ID}` section or a `class: generated` document. By convention `<dir>/<ID>.md`; the body is its goal, the title its H1 or `title:`. Written as any spec file (`spec propose create <dir>/<ID>.md`, `spec approve`).

```yaml
id: RM-0007
status: draft          # spec's: draft | in-progress | shipped | abandoned
release: v1            # optional: one of releases; absent: the pool
priority: should       # optional: one of priorities; absent: after every priority
estimate: 2-4          # optional: N or N-M in unit
links:
  depends_on: [RM-0004]   # optional: the chain
```

Typed (core `TYPED_KEYS` 30, model `Fields`, import's copy: `import-layout.md` "YAML"): `release`, `priority` `Text`; `estimate` `TextOrInteger`, a YAML integer kept as its decimal text (`3` -> `"3"`). Never counted (`node-revision.md` "Never counted"): a value of the key's type leaves `NORM_RECIPE`, every `norm_hash` and revision as they are. A wrongly typed one (a list, a mapping; a number or boolean under `release`, `priority`; a float or boolean under `estimate`) is `frontmatter-type`, kept in `extra`, and nulls its document's `norm_hash` as any wrongly typed key (`node-revision.md` "Null"), where it used to be an `unknown-key` warning.

No check rule of its own: `depends-cycle`, `ref-dangling`, `frontmatter-type`, `status-invalid`, `id-width`, `id-taken` cover items. Other reads see a node of its `[ids]` kind; a Tier 3 item only with `--archive`.

## Plan

Core `roadmap.rs`, `plan(&CheckInput, &RoadmapConfig) -> Plan`: pure, no clock, I/O or queue; one input in any order, one `Plan` (`PlanGroup`, `PlanItem`, `Sum`, `CutSum`, `Estimate`, `GroupKind`: the JSON's keys, plus `PlanItem.release_mistyped`; sums `u64`, saturating).

1. **Items**: an ID two documents hold: the first by path is the item.
2. **Done**: `status` `shipped` or `abandoned`; any other, absent or invalid too, is open.
3. **Groups**: each release in config order, then the pool (no `release`), then unplaced (a `release` not listed, or not a string); each listed, empty too, counting its done items and listing its open ones.
4. **Edges**: each `links.depends_on` entry resolving as the check resolves it (`Resolver`, `#section` ignored) to an item, itself included (a one-item cycle, its `waits_on` naming itself); one naming no item is ignored. `waits_on`: the targets still open, in written order, once each.
5. **Order in a release**: Tarjan's components of its open items over the edges between them, one of two or more items or with a self-edge a cycle; each after every one it depends on, of the ready ones the smallest key first. Key: (rank, ID), the rank `priority`'s index in `priorities`, their count when absent, off-scale or not a string; IDs as strings. A component's key is its smallest member's; members follow in key order, each `cycle: true`. `position` 1-based. Edges to other groups never reorder; they show in `waits_on`.
6. **Pool and unplaced**: by key, `position` null.
7. **Estimates** (`parse_estimate`): `N` -> N..N, `N-M` -> N..M; ASCII digits, each ≤ 1 000 000 (`ESTIMATE_MAX`), N ≤ M, no sign or space; `007` reads 7, `3-3` prints `3`. Other text or a mistyped value: unestimated, a note; absent: unestimated.
8. **Sums** per group over its open items: `min`, `max` the estimated bounds added, `unestimated` the others, never a 0. Cut: the releases from the first through `mvp_cut` (none without it); total: every release; pool and unplaced in neither.
9. **Current**: the first release holding an open item, else none.

## Notes

Of open items, each once, sorted by text: stderr `note:` lines after the index refresh's, and JSON `notes` (the plan's alone); never the exit.

- `<ID>: release "<r>" is not in [roadmap] releases`; `<ID>: release is not a string`
- `<ID>: priority "<p>" is not in [roadmap] priorities`; `<ID>: priority is not a string`
- `<ID>: estimate "<e>" is not N or N-M` (mistyped: `<e>` its compact JSON, `[1]`)
- `depends_on cycle: <ID>, <ID>…` (sorted)
- `<ID>: outside [roadmap] dir <dir>` (still an item)
- `<ID>: also defined in <path>; <path> is read`, when the file read or the one left out is open

## Command

`spec roadmap [--release R | --pool] [--json]`, CLI `roadmap.rs` (`roadmap(.., &RoadmapRequest {release, pool}) -> RoadmapOutcome`): reads the index the call updates, as `spec tree` (`corpus::indexed`), never the queue; writes only the data directory. `--release R`: that release's group; `--pool`: pool and unplaced; neither: every group. The first line and the JSON's top-level keys are always the whole plan's.

```text
roadmap: <o> open, <d> done; current <release or none>[; to <cut> <sum>]; total <sum>
release <name>[ | current][ | mvp cut] | <o> open, <d> done | <sum>
  <position>. <ID> | <title> | <status> | <priority> | <estimate>[ | waits on <ID>, …][ | cycle] | <path>
pool | <o> open, <d> done | <sum>
  - <ID> | … | <path>
unplaced | <o> open, <d> done | <sum>
  - <ID> | … | <path> | release <as written, or mistyped>
[truncated: <k> of <n> items not shown; add --release R or --pool]
```

An absent value prints `-`; `<sum>`: `<min>-<max>` (`<min>` when equal), ` <unit>` verbatim (`1 days`), `, <u> unestimated` when u > 0; text through the one-line rule. JSON, every key present, in order: `release`, `pool`, `reason`, `prefix`, `dir`, `unit`, `current`, `counts {open, done}`, `cut {release, min, max, unestimated}` or null, `total`, `groups [{group, name, current, mvp_cut, open, done, sum, items [{position, id, title, path, status, release, priority, estimate {min, max} or null, waits_on, cycle}]}]`, `notes`, `truncated`, `omitted`; `group` `release`, `pool` or `unplaced`, `name` null but for a release; an item's `release` as written, null when absent or mistyped.

Exits: 0 printed; 1 `--release R` naming no release: stderr `spec: no release "R" in [roadmap] releases (<list>)`, with `--json` the document, `reason` set, `groups` empty; 2: no project, a config error, `spec: <config> has no [roadmap]`, a bad table, an index not updatable, `--release` with `--pool` (usage).

## Cut

The tail prints its brackets literally. Before it at most 40 000 characters (`OUTPUT_CAP_CHARS`): lines, headers included, print whole and in order until the next would pass. `n` counts the items the request lists (with neither flag, pool and unplaced too), `k` those not shown; the tail and `truncated: true` only when k > 0. The JSON holds what prints: the groups whose header printed, their printed items, `omitted` = k.
