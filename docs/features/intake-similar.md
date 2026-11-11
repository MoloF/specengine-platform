---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-eval, plugin, scripts, fixtures, ui]
ref: RM-0059, v5 core slice 4 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 (section 1, RM-0059); the orchestrator's working answers, the owner's review open
adrs: []
---

# intake-similar: a create lists what it may repeat, and an exact twin waits to be named (RM-0059)

## Why

Today a writer's AI can propose "Rain ambience" beside an existing "Rain ambience", and nothing warns it: `create` only reserves IDs (`docs/canon/proposal-kinds.md` "Reservation"). Two live creates of the same thing both store, and the owner finds the twins in the Inbox. The intake already refuses an item until its exact hits are named (`docs/canon/agent-intake.md` "Dedup"); a create has no such step.

After this slice a create's answer lists `similar` entries, each with its source and reason: an equal title, an alias equal to the title, a live create's or amendment's title, a search rank, shared link targets. Only the exact three gate: the create stores once each gating entry is named in `distinct_from`, and the answer suggests an update of the existing node. Search and neighbour entries advise. The engine finds candidates deterministically; the agent's own model judges them (no LLM inside).

No ADR. ADR-0012: a refusal that naming passes is no block, as in the intake. ADR-0008: kinds are compared as opaque strings. ADR-0001: nothing new is stored, so no queue schema step. Vectors would need an ADR reversing 04 s2.1 and are out.

## Description and interactions

### Decisions

- **D1. Exact signals gate, the rest advise.** `title`, `alias` and `queue` entries carry `gates: true`; `search` and `neighbours` entries `gates: false`. A gate that hung on bm25 would differ between a dry run, two worktrees, a hub and a terminal (`docs/canon/spec-cli-read.md` "Known limits").
- **D2. Create only.** `spec propose create` and `propose_change` of kind `create`, both forms, on a terminal and on a hub's `/mcp`. An amendment, an update and an interpretation get `similar: null`.
- **D3. Not stored is an answer.** A create with an unnamed gating entry exits 0 with `id` `null` and a note, as the intake does (`docs/canon/agent-intake-documents.md` "Intake document"): no row, no event, no ID held.
- **D4. Nothing new stored.** `distinct_from` and `keywords` are read and dropped. A stored create's intake columns stay `NULL`, and `QUEUE_SCHEMA_VERSION` (12), the backup format and the index format stay as they are. The review document's `similar` is `null` everywhere but a propose's answer.
- **D5. Keywords per call.** `--keyword W` (MCP `keywords`) widens the search for this call only; no front-matter key, so no model or index change.
- **D6. One state, one list.** The order is fixed by sources and keys, never by rowid (`docs/canon/spec-index-edges.md`, E3). The dry run and the real call return byte-equal lists.
- **D7. A hub compares within its own store**: its checkout's index and its queue. A terminal's rows are not on it.
- **D8. Plugin 0.42.0**, provided RM-0105 `upstream-signal` (0.41.0) has shipped; else the next free MINOR, recorded here. `propose_change` takes two keys and returns one, so it is a MINOR. `propose-spec-change` and `write-flow` teach it.
- **D9. The UI gains one typed key and no behaviour.** The daemon serves `similar: null`.
- **D10. Measured, never asserted.** The 100 000-node timing (300 ms added) and recall on planted collisions at 10 000 nodes are `specengine-eval scale` numbers. Recall under 70 % opens the vector question for the owner; it fails nothing.

Every name in "Data" is fixed, so the lanes never wait on each other.

## Data

### Command

```text
spec propose create TARGET [--base HASH] --text-file F|- --rationale T
    [--author-role R] [--author-model M] [--run ID] [--brief] [--dry-run] [--task T]
    [--distinct-from X]... [--keyword W]...
```

- `--distinct-from X`: a gating entry's name, byte for byte: its `id`, else its `path`. At most 64 (`DISTINCT_MAX`), each 1-256 bytes with no control character: core's intake check, shared. A name that matches no entry is ignored.
- `--keyword W`: one search term. At most 16 (`KEYWORDS_MAX`), each 3-64 characters (`KEYWORD_MIN_CHARS` 3, equal to the store's `MIN_TERM_CHARS` by a compile-time assert; `KEYWORD_MAX_BYTES` 64 bytes), no whitespace or control character, each once.
- A cap broken: exit 2 `spec: distinct_from[i]: <problem>` or `spec: keywords[i]: <problem>`, no document, nothing read past the form (step 1 of `proposal-kinds.md` "Propose").
- The other `propose` kinds have no such flag: clap's exit 2.
- Exits: 0 stored, not stored or rehearsed; 1 refused (steps 2-5 as today, `similar` `null`); 2 as today plus the caps.

**The step.** After step 5 (validation) and before step 6 (insert), the similar step runs on the index this call refreshed (step 4) and on the queue. If every gating entry is named and there are at most 64, step 6 runs as today. Otherwise nothing is stored.

### The probes

Inputs, from the checked text (CLI `create.rs` `Checked`):
- **New nodes**: the file form's every parsed node; the section form's added `{#ID}` sections. Each is named by its ID, else (an ID-less document) by the create's path.
- **Titles**: each new node's parsed title, normalized as core `normalized_summary` (Unicode whitespace runs to one space, trimmed, `to_lowercase`); an empty one is skipped.
- **Kind and links**, the file form's document node only: its parsed kind, and its front-matter link targets of any type but `mentions`, canonical. `parent:` is not counted.

The corpus is the recorded root's worktree in the index, Tier 3 files dropped as `spec search` drops them without `archive`. The section form's target node is never an entry: the create extends it.

| # | Source | Gates | Finds | Order |
|---|---|---|---|---|
| 1 | `title` | yes | a node whose normalized title equals a new node's. Candidates: FTS5 on the `title` column with the title's terms of 3 or more characters ANDed and quoted as `search` quotes them, then a byte compare of the normalized forms. A title with no such term: a scan of the worktree's titles | path, then node order |
| 2 | `alias` | yes | a node with an `aliases:` entry whose normalized form equals a new node's normalized title; the worktree's aliases are compared in core | path, then node order |
| 3 | `queue` | yes | a live (`open`, `approved`) create or amendment of the project, any repository, adding a node whose normalized title equals a new node's. Its `new_text` is parsed under the recorded root's scheme: a new file's every node, a section form's nodes among its new IDs. A row that does not parse gives none | proposal ID number, then text order |
| 4 | `search` | no | the new titles' terms of 3 or more characters, then the keywords, lower-cased, each once, the first 32 (`SIMILAR_TERMS_MAX`), ORed. A term matching more than 1 000 nodes of the worktree (`SIMILAR_TERM_NODES_MAX`) is dropped. The top 5 (`SIMILAR_SEARCH_MAX`) | `bm25(nodes_fts, 10.0, 5.0, 1.0)`, path, node order |
| 5 | `neighbours` | no | nodes of the same kind as the new document (strings compared; no kind, no probe) holding front-matter edges (`edges`, origin `frontmatter`, type not `mentions`) to at least 2 (`SIMILAR_SHARED_MIN`) of its link targets. A target with more than 200 such edges (`SIMILAR_TARGET_DEGREE_MAX`) is skipped. The top 5 (`SIMILAR_NEIGHBOURS_MAX`) | shared count descending, path, node order |

The list is the five in that order. A node, or a proposal, already listed by an earlier probe is not listed again: its first source is kept, and so is the first new node it matched in text order. Gating entries beyond 64 are not listed; one note counts them.

### The entry

```json
{"id": "MEC-STAMINA", "path": "docs/spec/movement/stamina.md", "title": "Stamina", "kind": "mechanic",
 "source": "title", "reason": "title equal to new `MEC-ENDURANCE`'s", "gates": true}
```

- `id`: the node's ID (`null` for an ID-less document), or the proposal's `PR-` ID.
- `path`: the node's file, or the proposal's `target_path`.
- `title`, `kind`: as indexed, or as parsed from the proposal's text; `null` when none.
- `source`: `title | alias | queue | search | neighbours`.
- `reason`, one line; `<N>` is the matched new node's name; lists name at most 8, then `, and <k> more`:
  - title: ``title equal to new `<N>`'s``
  - alias: ``alias `<A>` equal to new `<N>`'s title``
  - queue: ``` `<PR>` (<status>), a live <kind>, adds `<M>` titled as new `<N>` ```, where `<kind>` is core's kind word and `<M>` the proposal's node ID, else its path
  - search: ``search rank <r> on `<t1>`, `<t2>` ``, over the terms kept
  - neighbours: ``same kind `<kind>`, <k> link targets shared: `<T1>`, `<T2>` ``

### The document

`ProposalDocument` (and its JSON, the daemon's `Proposal`) gains `similar` after `why`, before `notes`: a list on a create's propose answer (stored, not stored or rehearsed), `[]` when nothing is found; `null` on every other kind, on a refusal, in `spec review`, `get_proposal`, approve and reject, and on every daemon route. `distinct_from` stays `[]`, as the row holds none.

**Not stored**: the rehearsal's shape (`proposals.rs` `rehearsed`): `id`, `status`, `base_commit`, `preview`, `revisions`, `created_at`, `updated_at` `null`, `diagnostics` as they would be stored. The notes open with one of these two:

```text
not stored: a similar entry gates; propose an update of it, or name each gating entry in `distinct_from` (--distinct-from) to store it anyway
not stored: <n> similar entries gate and `distinct_from` names at most 64, so this create cannot be stored (a known limit)
```

**Dry run**: `dry run: nothing stored` first, then that note when the real call would store nothing.

### Text

`spec propose create` (and `--brief`, the MCP content). The head line is the ID, `dry run: nothing stored`, or the not-stored note. Then `introduced: <n>`, the finding lines, and one line per entry, controls escaped:

```text
not stored: a similar entry gates; propose an update of it, or name each gating entry in `distinct_from` (--distinct-from) to store it anyway
introduced: 0
similar: MEC-STAMINA | title | gates | mechanic | docs/spec/movement/stamina.md | Stamina | title equal to new `MEC-ENDURANCE`'s
similar: RULE-STAM-REGEN | search | advises | rule | docs/spec/movement/stamina.md | Regeneration | search rank 1 on `stamina`, `drained`
```

The line is `similar: <id or path> | <source> | gates|advises | <kind or -> | <path> | <title or -> | <reason>`; there are no lines when the list is empty. `--brief` cuts at 40 000 characters as today. `spec review` prints no `similar` line.

The example is `fixtures/spec-a` with a new `docs/spec/movement/endurance.md` (`id: MEC-ENDURANCE`, `title: STAMINA`, `# STAMINA`, `depends_on: [MEC-SPRINT]`, `derived_from: [R-12]`) and `--keyword drained`. The search row is illustrative: the run fixes it. MEC-STAMINA also shares two targets, but it is already listed by title.

### Library, store, core

- **core `similar.rs`**: `SimilarSource {Title, Alias, Queue, Search, Neighbours}` (`as_str`, `gates()`, an exhaustive match); `SimilarEntry {id, path, title, kind, source, reason, gates}` with `name()`; `keyword_problem`; and the constants above, `SIMILAR_GATING_MAX` = `DISTINCT_MAX`.
- **store**: `SpecIndex::similar(&SimilarQuery {titles: [{new, normalized}], terms, kind, links, exclude: Option<(path, ord)>}) -> Vec<SimilarRow {source, path, ord, id, title, kind, new, detail, rank}>`. These are probes 1, 2, 4 and 5 of the handle's worktree, ordered, deduplicated and capped; `detail` holds the alias, the kept terms, or the shared targets.
- **store**: `ProposalQueue::live_adds() -> Vec<LiveAdd {id, kind, status, target_path, new_text, new_ids, new_file}>`, read only, by proposal ID number, the rows `reserved()` reads.
- **CLI `similar.rs`**: builds the query from `Checked`, parses the live adds, inserts the queue entries after the alias ones, writes the reasons and applies the gate. `CreateRequest` gains `pub distinct_from: Vec<String>` and `pub keywords: Vec<String>`, and every construction sets them (`Vec::new()` where unused).

### MCP

`propose_change` (stdio `ChangeArgs`, hub `HubChangeArgs`) gains `distinct_from?: [string]` and `keywords?: [string]`, nullable, with property docs:
- `distinct_from`: "create only: each gating similar entry you judge distinct, by its id, else its path, byte for byte."
- `keywords`: "create only: extra search words, one word each."

A non-null value with another kind is -32602 ``distinct_from: only a create takes it, not an update`` (core's kind word, as `dry_run`'s refusal), nothing stored. A broken cap is an error result with the CLI's line. The output (`mirror.rs` `ReviewDocument`, also for `get_proposal` and `review_proposal`) gains `similar`: `null` or an array of `{id, path, title, kind, source, reason, gates}`, with `source` an enum of the five. `INSTRUCTIONS`, stdio and hub, are unchanged.

`CHANGE_DESCRIPTION` (2 044 → 2 041 B) and `HUB_CHANGE_DESCRIPTION` (2 040 → 2 037 B) take the same edits, ASCII, within 2 048:
1. `(a section and its subsections, or a whole file)` → `(a section with its subsections, or a file)`;
2. `create: new ID sections below the node (text: its span with them), or a new file at a free .md path (base null, text the whole file)` → `create: new ID sections below the node (its span with them), or a new file at a free .md path (base null)`;
3. after `refused naming the next free ID` (stdio: `a taken one is`; hub: `a taken or lower one is`), before its period, insert `; a similar entry with gates true stores nothing (id null) until distinct_from names it`;
4. `(not an alias, ID#SECTION or ID@rev), or a root-relative .md path (the whole file).` → `(no alias, ID#SECTION or ID@rev), or a root-relative .md path.`;
5. `why interpretation was needed` → `why it was needed`;
6. `to live nodes, which stay unwritten.` → `to live nodes, unwritten.`;
7. `base: get_node's span_hash of the target; a stale` → `base: get_node's span_hash; a stale`.

Every `holds_number` assert still holds.

### The daemon and the hub

- The daemon's key set `Proposal` has 51 keys, `similar` after `why`; it adds no set. No route changes.
- A hub's `/mcp/<slug>` `propose_change` takes the keys with no hub code beyond `HubChangeArgs`.

### The plugin

- `plugin.json` and `scripts/deploy/.env.example` go to 0.42.0. `PINS` and `SCHEMA_PINS` (`plugin_files.rs`) append 0.42.0 entries.
- **`propose-spec-change`**, body 4 092 → 4 088 B (the `spec_ids` test's 4 092 bar), file 4 378 → 4 374; the description is unchanged:
  1. step 2 → ``2. A cut answer: target a smaller ID section, or read the file, `line` to `end_line`.``;
  2. `the text its span plus the new ID headings below its own level, its own ID headings kept at their levels.` → `the text its span plus new ID headings below its own level.`;
  3. `` `target` a root-relative .md path where nothing is yet, inside the spec's directories; `` → `` `target` a free root-relative .md path inside the spec's directories; ``;
  4. after the `- Your new IDs are held` bullet, a new bullet: ``- The reply's `similar` lists what yours may repeat, with why: prefer an update of it. One with `gates` true keeps yours unstored (`id` null) until you resend naming it in `distinct_from`.``;
  5. ``- Refused for a stale `base`: the span changed since you read it; read it again, redo your text, resend.`` → ``- Refused for a stale `base`: read the span again, redo your text, resend.``;
  6. ``- Refused for the span's headings: keep its ID headings at their levels, a new heading below the section's; an ID section`` → ``- Refused for the span's headings: an ID section``.
- **`write-flow`**, body 4 086 → 4 089 B, file 4 155 → 4 158; the description, five steps, facts and eight blocks are unchanged:
  1. drop `` `get_flow` reads it back: its rows name what is missing and how to clear it.``, with the space before it;
  2. `a proposal needs a commit and the rules live there.` → `a proposal needs a commit.`;
  3. step 3 ends with: ``A reply with `id` null names a gating `similar` entry: reuse it, or resend naming it in `distinct_from`.``

### The UI line

- `ui/src/api/provisional.ts`: `Proposal` gains `similar: null;` after `why`, with the doc comment: "Always null here: only a create's propose answer lists it (`docs/features/intake-similar.md` "Acceptance criteria", IS10)."
- `ui/src/test/builders.ts` and `ui/src/mocks/build.ts`: each `Proposal` built gains `similar: null`.
- `ui/src/api/daemonKeys.test.ts`: `KEYS.Proposal` and its `CITED` string gain `similar` after `why`, and the header names this slice. While the fixture lacks the key, `AHEAD_OF_FIXTURE` holds `["Proposal", "similar"]`.

### Eval

`specengine-eval scale` (`docs/canon/scale-eval.md`) gains, per size:
- **`reads.similar {median_us, max_us}`** and `growth.similar`: `SpecIndex::similar` on the warm index for the first 3 W targets. Each query is the target's title upper-cased, its rare word as a keyword, kind `node`, and its document's `depends_on`; timed as the reads are, with no steps or trusted twin. RM-0059's bar is a median within 300 ms at 100 000.
- **`similar_recall {planted, found, variants: {exact, reordered, dropped, keyword: {planted, found}}}`**: 100 collisions, 25 per variant, on live sections whose titles have 3 or more words, drawn from the size's stream. `exact` uses the title in another case; `reordered` its words reversed; `dropped` its last word dropped; `keyword` two common words absent from the title, plus the section's rare word as a keyword. A collision is found when its target is among the rows. stdout holds numbers only.

## Rules and edge cases

- WHEN a create's new node has a title equal to a live corpus node's, to an alias, or to a node a live create or amendment adds, the create SHALL store only if `distinct_from` names each such entry. The findings SHALL be computed and shown either way.
- WHEN only search or neighbour entries exist, the create SHALL store as today.
- WHEN not stored, there SHALL be no row, no event and no reserved ID; the queue's row count and last event `seq` are unchanged.
- WHEN a create is refused at steps 1-5, `similar` SHALL be `null` and no probe SHALL run.
- WHEN the agent resends its own earlier create (a cancelled call that stored), that row SHALL gate. The skill's way out is to name it in the report and ask for a reject, never to name it distinct.
- The section form's target and Tier 3 files SHALL never be entries. A new node without a title SHALL give no title, alias or queue probe; a new document without a kind, no neighbour probe.
- Determinism: one index and queue state gives one list and one text. Copies at other roots and `HOME`s are byte-equal; no rowid order.
- Writes: as a create today (the index refresh, the row when stored). The probes write nothing.
- ADR-0012: nothing gates but an exact twin, and naming it passes. No other command reads `similar`.

### Rollout

- An older `specengine-mcp` on a terminal, or a hub at 0.38, refuses `distinct_from` and `keywords` (`unknown field`). The hub picks them up at its next upgrade, the owner's call.
- No queue schema step, backup format, index format, config key or route.

### Test churn

- Every pinned review-document key list (CLI, MCP schema walkers, http `daemon_keys.rs`) gains `similar` (51 keys).
- Every pinned `propose create` text or JSON whose create now meets an entry gains its `similar` lines or array. The test-engineer checks each entry's reason before re-pinning.
- A test that stores a second create with the title of a live one or an indexed node names it in `distinct_from`, or retitles. Reservation tests keep their step 4 refusal, since step 4 refuses before similar runs.
- `CreateRequest` constructions in CLI and MCP tests gain the two fields. Plugin pins: `plugin_skills.rs` sizes and phrases, `plugin_files.rs`, `plugin_roles.rs`, http `spec_ids_hub.rs` (`v0.42.0`), and `mcp_hub.rs`'s schemas if it pins them.

## Acceptance criteria

A browser look is **[look]**, a timed or counted run **[measure]**; every other criterion is a test that the named mutation turns red. New Rust tests: `intake_similar.rs` in `crates/specengine-cli/tests` and `crates/specengine-mcp/tests`, over scratch git copies of `fixtures/spec-a`; core and store units in `#[cfg(test)]`.

- [ ] IS1 — **The title gate.** The "Text" example's create: exit 0, `id` `null`, `similar[0]` exactly the "The entry" example, the not-stored note first in `notes` and as the head line. The proposal row count, the last event `seq` and `reserved()` are unchanged, and another create of `MEC-ENDURANCE` is not refused as reserved. With `--distinct-from MEC-STAMINA` it stores `PR-0001`, its `similar` byte-equal to the first call's. Mutation, red: the title probe skipped.
- [ ] IS2 — **The queue gate.** Create A, a new file titled "Lantern oil", stores `PR-0001`. Create B, another path and ID with the title "lantern  OIL", lists `{id: "PR-0001", source: "queue", gates: true}` and is not stored. After `spec reject PR-0001 --reason x`, B stores with no entry. A live amendment whose file is titled "Lantern oil" gates alike. Mutations, each red: the live-create probe dropped; amendments not read.
- [ ] IS3 — **The alias gate.** The copy gains a document with `aliases: ["Old lamp"]`, committed. A create titled "Old Lamp" lists that document with `source: alias`, gating. Mutation, red: aliases not probed.
- [ ] IS4 — **Advice never gates.** A create titled "Stamina regeneration curve" with `--keyword drained` lists only `search` entries, each `gates: false`, at most 5, and stores with no `distinct_from`. A mechanic titled "Endurance pool" with `depends_on: [MEC-SPRINT]` and `derived_from: [R-12]` lists MEC-STAMINA as `neighbours`, its reason naming both targets, and stores. With every term common (the cap set to 1 in a unit test) there are no search entries. `write-flow`'s eight creates still store with no `distinct_from` on `flow_authoring.rs`'s project. Mutation, red: search entries gating.
- [ ] IS5 — **Order and determinism.** Two copy documents `docs/spec/a.md` and `docs/spec/b.md` share a title. `a.md` is edited after the first index, so its rows are newer. A create of that title lists `a.md`, then `b.md`. The dry run and the real call give byte-equal `similar` (JSON value and text lines), and so does a copy at another root and `HOME`. A section-form create never lists its target. A Tier 3 document with the title is not listed. Mutations, each red: order by rowid; the target not excluded.
- [ ] IS6 — **Kinds, flags, caps.** Over MCP, `distinct_from` or `keywords` with `update`, `interpretation` or `amendment` gives -32602 naming the field and the kind, with nothing stored. `spec propose update … --distinct-from X` exits 2 (clap). Update, interpretation and amendment answers, `spec review`, `get_proposal`, approve and reject carry `similar: null`, and the review text has no `similar` line. Each of these exits 2 naming the field and its index: `--keyword ab`, 17 keywords, a keyword with a space, a keyword twice, 65 `--distinct-from`, a 257-byte one. A unit test with 65 gating entries gives the unnameable note and lists 64. Mutation, red: `distinct_from` accepted on an update.
- [ ] IS7 — **Nothing new stored.** A create stored with `--distinct-from` and two `--keyword`s has a row equal, column by column apart from `id` and the times, to the same create stored after the corpus node is renamed (no entry). Its intake columns are `NULL`. `QUEUE_SCHEMA_VERSION` is 12, and `spec export state` writes the backup format of before, the row's line with the same keys. Mutation, red: `distinct_from` stored.
- [ ] IS8 — **Parity, schemas, texts, the hub.** The stdio `propose_change {kind: "create", …, distinct_from, keywords}` content is byte-equal to `spec propose create … --brief 2>&1`, and its `structuredContent` equals `--brief --json`; the same holds with `dry_run: true` and when not stored. The schema walker passes with exact key sets: input `distinct_from` and `keywords`, `additionalProperties: false`; output `similar` items `{id, path, title, kind, source, reason, gates}`. Both descriptions are as "MCP" gives them (2 041 and 2 037 B), and `INSTRUCTIONS`, stdio and hub, are byte-equal to before. On a hub's `/mcp/<slug>` the IS1 call answers alike and `tools/list` holds 18 tools. Mutations, each red: `reason` dropped from the mirror's entry; `distinct_from` ignored on the hub's route.
- [ ] IS9 — **Genre.** `proposal_genre.rs` `NEW_SOURCES` gains core and CLI `similar.rs`: no kind, prefix, `docs/` or `records/` literal. The kind words come from core constants, and no dependency named `similar` is added (the existing guard). Mutation, red: a `"create"` literal in CLI `similar.rs`.
- [ ] IS10 — **The daemon serves null; the UI types it.** `GET …/proposals/PR-0001` serves `similar: null` among 51 keys, and `fixtures/daemon-keys.json` `Proposal` names `similar` after `why`. `provisional.ts` has `Proposal.similar: null`, citing this criterion. UI lint has 0 problems, `tsc` is clean, and `pnpm test` is green, with `AHEAD_OF_FIXTURE` empty at the end. Mutations, each red: `similar` out of the fixture; a route serving a list.
- [ ] IS11 — **Plugin 0.42.0 and the skills.** `plugin.json` is 0.42.0, `.env.example` has `SPECENGINE_VERSION=0.42.0`, and `tag-check.sh v0.42.0` prints it. `PINS` and `SCHEMA_PINS` end with 0.42.0. The skills carry "The plugin"'s edits, each once, with the sizes given, no digit outside a step number, and the descriptions unchanged; AC-06, AC-07, AC-08 and AC-11 pass. Mutations, each red: the schema changed with no `SCHEMA_PINS` entry; the `similar` bullet dropped; `distinct_from` unbackticked in `write-flow`.
- [ ] IS12 — **[measure] Time and recall.** `specengine-eval scale` reports `reads.similar` and `similar_recall` at 1k, 10k and 100k (release, seed 1). The 100k median and the 10k recall, by variant, are recorded in "Implementation". `scale_cli.rs` asserts that the keys exist and that `exact` finds 25 of 25 at 1k; it never asserts a time or the overall recall. A 100k median over 300 ms, or a 10k recall under 70 %, is reported to the owner and opens the vector question (the latter); it is no red. The owner's look at pilot A is his own and is reported anonymously.
- [ ] IS13 — **Gates.** `cargo nextest run` for `specengine-core`, `-store`, `-cli`, `-mcp`, `-http` and `-eval` is green, then the full run once. The eval tests (`doc_pointers`, `ui_policy`, `anonymity`) and `spec check` pass. UI lint, `tsc`, `pnpm test` and `pnpm build` pass, with first-load JS unchanged within 0.1 kB.

## Out of scope

Out: vectors and a stemmer (each needs an ADR), a `keywords:` front-matter key, `similar` on an update, interpretation or amendment, a stored `distinct_from`, a UI that shows `similar`, the intake's own dedup (exact and unchanged), comparing a hub's queue with a terminal's, and RM-0060's collision pass.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | core `similar.rs`, `lib.rs`; store `read.rs` (or a new `similar.rs`), `lib.rs`, `queue.rs` (`live_adds`); CLI `similar.rs` (new), `create.rs`, `proposals.rs`, `main.rs`, `lib.rs`; MCP `intake.rs`, `hub.rs`, `mirror.rs`; eval `src/scale/`; `plugin/specengine/.claude-plugin/plugin.json`; the two `SKILL.md`; `scripts/deploy/.env.example` |
| test-engineer, Rust | `intake_similar.rs` in CLI and MCP (new); core and store units; `proposal_genre.rs`; the churn; plugin tests; http `daemon_keys.rs`, `spec_ids_hub.rs`; eval `scale_cli.rs`; `fixtures/daemon-keys.json` (`SPECENGINE_WRITE_DAEMON_KEYS=1` once the Rust compiles), removing `["Proposal", "similar"]` from `AHEAD_OF_FIXTURE` in the same change |
| ui-developer | "The UI line"'s four files, nothing else |

Comments cite `docs/features/intake-similar.md` "Acceptance criteria" with a criterion ID, or a canon heading; never "Data", which compaction drops (`doc_pointers`).

### Work order

1. Start after RM-0105 `upstream-signal` ships: one Rust slice at a time, and the plugin version follows it.
2. The ui-developer adds the UI line behind `AHEAD_OF_FIXTURE`.
3. The rust-developer works through core, store, CLI, MCP, the eval, then the texts. The Rust test-engineer overlaps: IS1-IS7, then IS8-IS11 with the churn, then IS12's run. Then the Rust review.
4. Ship: the spec-writer ("Canon at ship").

### Risks

- **Churn** in pinned create texts: every create meeting a search entry gains lines.
- **The race**: the live-create probe reads outside the write lock, so two creates of one title in the same instant may both store (W4).
- **Text room**: `propose-spec-change` 4 088 of 4 092, `write-flow` 4 089 of 4 096, the descriptions 2 041 and 2 037 of 2 048.
- **Time**: the term-frequency counts and the edge reads at 100k, unknown until IS12.

### Canon at ship

Free bytes now are in brackets. "Net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/proposal-similar.md` (new, Tier 2, the `proposal-*` shard) | the step, the five probes, the entry, the gate, the notes, the text line, MCP, the hub, Known limits (the race, FTS case folding, Tier 3 unseen, another scheme's rows), Tests, the measured numbers |
| `docs/canon/proposal-kinds.md` [32] | "Propose": the similar step between 5 and 6, a pointer; "MCP and plugin": the two keys (net not positive) |
| `docs/canon/agent-intake-documents.md` | "Review document": `similar` after `why`, `null` but on a create's answer |
| `docs/canon/hub-mcp.md` [24], `docs/canon/scale-eval.md` [3] | the keys on a hub's `propose_change`; the `similar` read and `similar_recall` (each net not positive) |
| `docs/canon/plugin-files.md`, `docs/canon/plugin.md` [316] | **Version** current 0.42.0, with what it added; `propose-spec-change`'s and `write-flow`'s sizes |
| crate READMEs (core [9], store [3], CLI [1], MCP [7]) | a module line each where they list modules, net not positive |
| `CLAUDE.md`; 08 s2 Phase 7 "Owner's word, 2026-11-12" row 4; RM-0059 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner did not answer: D1, D2 (an amendment gets none; a hub compares within its store), D4, D5, D10.

**The spec-writer's:**
- W1: the text line format, and the head line on a not-stored answer;
- W2: `similar` is `null` in reviews, and the review text has no line for it;
- W3: Tier 3 files and the section form's target are never entries;
- W4: the live-create probe reads outside the write lock (Q-E);
- W5: the caps 5, 5, 32, 1 000, 200 and 2, tunable on IS12's numbers;
- W6: caps broken exit 2 with no document, as `propose update`'s author grammar does;
- W7: unknown `distinct_from` names are ignored;
- W8: the kind does not narrow the title gate (Q-D);
- W9: titles under 3 characters are scanned;
- W10: the UI line goes first, behind `AHEAD_OF_FIXTURE`.

**The owner's questions:**
- **Q-A.** Do search and neighbour hits refuse a create until named? Working answer: no. The cost of yes: a second call per common-word create, and a gate that differs by DB, worktree, hub and terminal.
- **Q-B.** `keywords` per call, or a front-matter key? Working answer: per call; a key only if recall stays under 70 %. The cost of a key: a model change, an index format step, and an older `spec check`'s reaction is unverified.
- **Q-C.** How is recall measured? Working answer: planted collisions in a seeded 10k corpus, plus his look at one pilot. The cost of labelled pilot duplicates: his hour, and the report must stay anonymous.
- **Q-D.** Should an equal title of another kind gate? Working answer: yes, any kind; the entry shows the kind. The cost of same-kind only: a mechanic and a rule both titled "Stamina" pass silently.
- **Q-E.** Should the live-create probe run under the write lock? Working answer: no, it is a known limit. The cost of yes: the store parses live rows' texts under the lock with the caller's scheme, a new store-core contract.
