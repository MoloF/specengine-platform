---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-eval, plugin]
owner: owner
reviewed: 2026-11-13
---

# Collision pass: pairs that may collide, an agent's verdicts, a local ledger

Built by `collision-pass` (RM-0060) and `collision-names`, no ADR: the ledger is operational state in the queue DB, never in git (ADR-0001, ADR-0003); a mark decides and blocks nothing (ADR-0012); kinds are opaque strings, `mentions` the one link type named (ADR-0008). The engine lists candidate pairs from links and titles; an agent judges each, files a real collision (a duplicate too) as `report_discrepancy` with `gap_type: contradicts`, and marks it; a pair returns only when a side's meaning changes. Code: core `collisions.rs`; store `read.rs` (`SpecIndex::collision_input`), `queue/collisions.rs`; CLI, MCP `collisions.rs`; eval `src/scale/`; plugin `skills/check-collisions/`.

## Command

```text
spec collisions [--kind K]... [--any-kind] [--limit N] [--json]
spec collisions mark --input F|- [--author-role R [--author-model M] [--run ID]] [--json]
```

- `--kind K`, repeatable, keeps those kinds' groups, never an error; `--any-kind`: one group across kinds (with `--kind`: of the kinds named).
- `--limit` 1 to 200 (`COLLISIONS_LIMIT_MAX`), default 50 (`COLLISIONS_LIMIT_DEFAULT`), else exit 2 `spec: --limit <n>: the limit is 1 to 200`; the help is built from the constants.
- The read exits 0 (zero pairs too) or 2, writing only the index. Mark exits 0, 1 refused, 2 a form error; `--author-model`, `--run` need `--author-role`.

## Universe

The root's worktree in the index, live files only (`spec-cli-graph.md` "Live sources"): documents and ID sections with a kind, all of them an ID's holders whatever `--kind`. A node's name is its canonical reference (core `feature_stem`, `feature_scoped`, as the resolver): in a feature document `slug/ID` when the ID is feature-scoped or another node holds it (a repeated `R-97` is `feat/R-97`, `canonical_id` gives `R-97`); else the bare ID; an ID-less document its path. **W1:** one outside feature documents whose ID another node holds has no name, is not paired and is noted; a feature holder keeps `slug/ID`. One with no `norm_hash` (its file does not parse whole) is not paired, counted in `skipped` (kept kinds only). Never paired: two holders of one name (repeats in one feature document); a document with its own sections.

## Signals and score

Pairs are drawn within a kind, compared byte for byte; every cap counts within the group, so `--kind` never changes a kind's pairs.

- **(a) Shared targets.** A node's targets: the resolved ends of the `edges` rows it holds (`names_source` 0) of any type but `mentions`, and its resolved `edge_parents` ends (`uses`, `parent:`, a shared term). A target held by more than 64 nodes of the group (`COLLISION_TARGET_DEGREE_MAX`) is skipped. `shared`: the targets both hold.
- **(b) Titles.** The title as core `normalized_summary` gives it, padded with a space each side; its trigrams, windows of three Unicode scalar values, as a set. `titles` = floor(2000 × |A∩B| / (|A|+|B|)) over each title's full set, 0 when both are empty: Dice per mille; capped trigrams (below) count too.
- **(b)'s candidates.** Per member, the postings of the trigrams at most 200 titles of the group hold (`COLLISION_TRIGRAM_DF_MAX`), partners only later in `(path, ord)`; and its words bucket, keyed by `normalized_summary(title)` split at Unicode whitespace, sorted byte-wise, deduplicated, joined by one space (no word, no bucket), a bucket of 2 to 200 titles. A title-only candidate is skipped when floor(2000 × min(|A|,|B|) / (|A|+|B|)) < 500, a true upper bound; a pair sharing a target always gets its exact `titles`.
- **Qualifies:** `shared` >= 1 or `titles` >= 500 (`COLLISION_TITLES_MIN`). **Score:** `titles + 200 × min(shared, 5)` (`COLLISION_SHARED_WEIGHT`, `COLLISION_SHARED_SCORED_MAX`), 200 to 2 000.
- **Order.** `a` is the side first by `(path, ord)`; pairs by score descending, then `a`'s `(path, ord)`, then `b`'s. A bounded heap keeps the top `limit`; `left` = qualifying − listed.
- **The ledger filter**, before the limit: a pair is hidden while the project's row for its two names holds both current norms. No pair is stored.

## Document and text

```json
{"project": "lantern-keep", "kinds": [], "any_kind": false, "limit": 50,
 "pairs": [{
   "a": {"id": "MEC-ENDURANCE", "path": "docs/spec/movement/endurance.md", "ord": 0, "kind": "mechanic", "title": "Stamina pool", "norm_hash": "b3:5e1f…"},
   "b": {"id": "MEC-STAMINA", "path": "docs/spec/movement/stamina.md", …},
   "score": 936, "titles": 736, "shared": 1, "shared_targets": ["MEC-SPRINT"], "marked": null,
   "reason": "titles alike 0.736; 1 link target(s) shared: `MEC-SPRINT`"}],
 "left": 0, "skipped": 0, "notes": []}
```

- `id` the name, null for an ID-less document; `title` null when none. `shared_targets`: names in `(path, ord)` order, at most 8 (`COLLISION_SHARED_LISTED_MAX`). `marked`: null, or `{verdict, proposal, at}` of a row whose norms no longer hold.
- `reason`, parts joined by `; `: `titles alike <d.ddd>` (per mille, 1000 as `1.000`) when `titles` >= 500; ``<n> link target(s) shared: `T1`, `T2` `` when `shared` >= 1, at most 8 named, then `, and <k> more`; `marked <verdict> <at>, changed since`.
- `notes` (`note:` lines on stderr, first in `2>&1`), in order: ``<n> node(s) without a norm_hash not paired: their files do not parse whole``; per W1 ID, byte order, ``<n> node(s) outside feature documents not paired: their ID `<ID>` is held by more than one node (`spec check` reports it)``, `n` its kept kinds' nodes with a norm; an unreadable ledger row.
- Text: `collisions: <n> pair(s) listed, <left> left, <skipped> node(s) skipped`, then per pair `<score> | <a name> | <b name> | <a kind>[/<b kind>] | <reason>`, controls escaped.
- **The cap**, 40 000 characters (`OUTPUT_CAP_CHARS`): pairs whose lines do not fit after the head line leave text and JSON alike, counted in `left`, noted `<k> pair(s) past the 40000-character cap counted in left: judge the pairs listed, then call again`. So `structuredContent` equals `--json`.

## Mark

Input `{"pairs": [{"a", "a_norm", "b", "b_norm", "verdict", "proposal"}]}`, unknown fields refused. In order; the first refusal answers, its reason `spec: <reason>` on stderr and alone in JSON `notes`, nothing written:
1. Form, exit 2: clap, the MCP schema, `--input` not UTF-8 JSON of the shape, an unknown `verdict`.
2. Caps, exit 1, prefixed `pairs[i].<field>: ` (core `judgement_problem`): `pairs` 1-200 (`COLLISION_MARKS_MAX`); a name 1-256 bytes (`COLLISION_NAME_MAX`), no control character; `a` equal to `b`: `pairs[i]: a pair of one node`; a norm `b3:` plus 64 lower-case hex; `reported` names a `PR-` `proposal`, `distinct` none; a pair repeated in either order: `pairs[i]: repeats pairs[j]`; then the author (`agent-intake.md` "Authors").
3. Each `proposal` in the project's queue, any state, else `pairs[i].proposal: no proposal PR-0099`.
4. Each name a node of the universe as the read names it, no alias, else `pairs[i].a: ` and, holders' forms in path order: `` `AC-01` is feature-scoped: cite it as `one/AC-01` or `two/AC-01` ``; `` `AC-07` is feature-scoped and no feature document defines it ``; `` `R-97` is held by more than one node: cite it as `feat/R-97` or `other/R-97` ``; `` `R-01` is held by more than one node outside feature documents, never paired (`spec check` reports it) `` (every holder W1); ``no live node `X` ``. A side whose norm no node of that name holds now is a note, never a refusal: `pairs[i]: <name> changed since it was listed; the pair will be listed again`.
5. One `Immediate` transaction upserts a row per pair: no event, no proposal touched.

The answer is `{marked, replaced, notes}`; its text `marked <n> pair(s), <r> replacing an earlier mark`, then the notes.

## Ledger

Queue schema 14 (`STEP_14`, in `migrate`'s one transaction): `collision_marks (project, a, a_norm, b, b_norm, verdict, proposal, author, at, PRIMARY KEY (project, a, b)) STRICT`, every column `TEXT`, only `proposal` nullable. `a` < `b` byte-wise, each norm after its name; `author` core `Author` JSON (`human` without `--author-role`); `at` the call's clock.

- **Ops**, inherent `SqliteQueue` methods, store `queue/collisions.rs`: `collision_marks() -> CollisionMarks {marks, unreadable}`, the project's rows (none below schema 14); `mark_collisions(&[NewCollisionMark], &Author, now) -> MarkResult {marked, replaced}`; `proposal_exists(id)`. A write below 14: `Invalid`, unreachable from the CLI.
- **Unreadable rows** (`a` >= `b`, a bad norm or verdict, `reported` without a proposal, a value not UTF-8 or not text) are ignored, noted: `collision mark <a> <b>: unreadable (<column>: <reason>); ignored`.
- **Never dumped:** `export state` writes the five tables only; format 7 holds schema 13 or 14 (`queue-backup.md`). A restore empties the project's ledger inside its transaction (`clear_ledger`, `WHERE project`), another project's in a shared DB kept.

## MCP, hub, plugin

- `get_collisions {kinds?, any_kind?, limit?}` is `spec collisions`: the reads' annotations and `_meta`, output mirror `CollisionsDocument`. `mark_pairs {pairs: [{a, a_norm, b, b_norm, verdict, proposal?}], author_role, author_model?, run?}` is `spec collisions mark --input -`: the queue writers' annotations, `MarkDocument`, `verdict` an enum, items closed.
- `INSTRUCTIONS` name both: `- get_collisions, mark_pairs = spec collisions, collisions mark.` (`mcp-read.md` "Texts"). The stdio server lists 32 tools.
- **A hub** serves neither (a local cache), both in `LEFT_OUT` (`hub-mcp.md` "Tools"); its queue migrates to 14, the ledger empty.
- **Plugin 0.44.0**, the eighth skill `check-collisions`, its steps in `plugin.md` "Skills".

## Measured

`specengine-eval scale` (`scale-eval.md`): `reads.collisions` (default flags, an empty ledger, the warm index) and `collision_planted {planted, found}`: 25 title twins per size, a live section's title copied onto another of its kind, 12 with words reversed, all with case changed, found when among `--limit 200`'s pairs. Release, seed 1, beside the test suites: medians 1k 16.9 ms, 10k 285 ms, 100k 274 ms, under 2 s; twins 25/25, 25/25, 0/25, the 100k generator's 1 296 two-word titles giving 44k identical pairs at `titles` 1000 that rightly come first.

On a scratch copy of a real game corpus, default `--limit`: 8 502 pairs left, assumptions 5 210 and questions 2 195 from one shared target; criteria 862, title-only, mostly recurring boilerplate titles, 927 under `collision-names` (one ID's holders in two features pair; a pair's bundle 2 nodes, the bare `AC-16` 27; no W1 node).

## Known limits

- A W1 end in `shared_targets` stays bare. Tier 3 and generated duplicates of a live bare ID are no holders: `spec bundle <bare>` may return an archived second node. Per-ID notes are outside the 40 000-character cap.
- **One shared target qualifies a pair**, flooding a real project (above): whether to require more is the owner's open question.
- The 200 cap is absolute per group: recall in a huge one-kind group rests on title variety.
- Trigram sets ignore repeated words: `Ky qy qy` and `Ky qy` score 1000.
- The shared-target map is built whole per group, its peak the sum over targets of C(min(degree, 64), 2).
- Trailing whitespace, blank-line runs and a heading-only rename change no `norm_hash` (`node-revision.md` "The recipe"): the pair stays hidden. A renamed node pairs afresh, as a pair marked by a bare name before `collision-names`; a node losing its kind, its whole parse or its tier leaves the universe; their old marks stay, unread.
- A node may pair with an enclosing ID section.
- Older builds refuse a schema 14 queue and a dump at 14; the hook gives no verdict until a reinstall; a hub dumps first.

## Tests

CLI and MCP `tests/collisions.rs` on scratch copies of `fixtures/spec-a`; core and store units; eval `collision_ban.rs` (the ledger's three names in nine files only) and `scale_cli.rs` (keys, 25 of 25 at 1k, no time); the plugin tests. Criteria: `docs/features/collision-pass.md`, `docs/features/collision-names.md` "Acceptance criteria".
