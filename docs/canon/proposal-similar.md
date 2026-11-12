---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-eval, plugin]
owner: owner
reviewed: 2026-11-12
---

# Proposal similar: what a create may repeat

Built by `intake-similar` (RM-0059, plugin 0.42.0), no ADR: a refusal that naming passes blocks nothing (ADR-0012), as the intake's dedup (`agent-intake.md` "Dedup"); kinds are opaque strings (ADR-0008); nothing new is stored (ADR-0001). Vectors and a stemmer each need an ADR (04 s2.1). The engine finds candidates deterministically; the agent's model judges them. Related: `proposal-kinds.md` "Propose", `agent-intake-documents.md` "Review document". Code: core `similar.rs` (`SimilarSource`, `SimilarEntry`, the constants, `similar_terms`, `term_words`, `named_list`, `keyword_problem`, `distinct_from_problem`); store `similar.rs` (`SpecIndex::similar`), `queue.rs` (`ProposalQueue::live_adds`); CLI `similar.rs`, `create.rs`, `proposals.rs`; MCP `intake.rs`, `hub.rs`, `mirror.rs` (`SimilarShape`); eval `src/scale/`.

## The step

`spec propose create … [--distinct-from X]... [--keyword W]...`; no other kind takes them (clap, exit 2).

- **Form** (step 1, exit 2, no document, nothing read past it): `distinct_from` at most 64 (`DISTINCT_MAX`), each 1-256 bytes with no control character (core's intake check, shared); `keywords` at most 16 (`KEYWORDS_MAX`), each 3-64 (`KEYWORD_MIN_CHARS`, equal to the store's `MIN_TERM_CHARS` by a compile-time assert; `KEYWORD_MAX_BYTES` bytes), no whitespace or control character, each once. The line names the field and its index, the count cap the first past it: `spec: distinct_from[64]: 65 entries; at most 64`, `spec: distinct_from[0]: 257 bytes; at most 256`, `spec: keywords[16]: 17 entries; at most 16`, `spec: keywords[0]: 2 character(s); at least 3`, `…: holds whitespace: one word each`, `…: holds a control character`, `…: 65 bytes; at most 64`, `spec: keywords[1]: repeats keywords[0]: give each once`.
- **Probes**, after validation (step 5), before the insert (step 6), on the index step 4 refreshed and on the queue. When every gating entry is named and there are at most 64 (`SIMILAR_GATING_MAX`), step 6 runs as before; else nothing is stored: exit 0, `id` `null`, no row, no event, no ID held. A refusal at steps 1-5: `similar` `null`, no probe run.
- **Naming**: `distinct_from` names an entry byte for byte, its `id`, else its `path`; a name matching none is ignored. Both lists are read and dropped: a stored create's intake columns stay `NULL`, with no queue schema, backup or index format step. `keywords` widen this call's search only.
- **Only the exact three gate** (`title`, `alias`, `queue`; `SimilarSource::gates`, an exhaustive match); `search` and `neighbours` advise, since a gate on bm25 would differ between a dry run, worktrees, a hub and a terminal. Any kind's equal title gates; the entry shows the kind.

## The probes

From the checked text: the **new nodes** (the file form's every parsed node, the section form's added `{#ID}` sections), each named by its canonical ID (`slug/ID` for a feature-scoped prefix, as `target_ids`), else the create's path; their **titles**, normalized as core `normalized_summary` (whitespace runs to one space, trimmed, `to_lowercase`), an empty one skipped; for the file form's document, its **kind** and its own front-matter links of any type but `mentions` (no `superseded-by` source's), `parent:` not counted. **Terms** (`similar_terms`): the titles' words, then the keywords, split at Unicode whitespace and at U+0000 (`term_words`), lower-cased, of at least 3 characters, each once, the first 32 (`SIMILAR_TERMS_MAX`).

The corpus is the recorded root's worktree in the index, Tier 3 files dropped; the section form's target is never an entry. Probes 1, 2, 4 and 5 run in one deferred store read, writing nothing; 3 is the CLI's.

| # | Source | Finds | Order |
|---|---|---|---|
| 1 | `title` | a node whose normalized title equals a new one's: candidates by FTS5 on the `title` column, the title's terms quoted and ANDed, then a byte compare of the normalized forms; a title with no term of 3 characters: a scan of the worktree's titles, once a call | path, node order |
| 2 | `alias` | a node with an `aliases:` entry whose normalized form equals a new normalized title, compared in core | path, node order |
| 3 | `queue` | a live (`open`, `approved`) create or amendment of the project, any repository (`live_adds()`, by ID number), adding a node whose normalized title equals a new one's; its `new_text` parsed under the recorded root's scheme, a new file's every node, a section form's among its `new_ids`; a row that does not parse gives none | proposal number, text order |
| 4 | `search` | the terms ORed; a term matching more than 1 000 worktree nodes (`SIMILAR_TERM_NODES_MAX`, counted exactly with `LIMIT 1001`) dropped; the top 5 (`SIMILAR_SEARCH_MAX`) | `bm25(nodes_fts, 10.0, 5.0, 1.0)`, path, node order |
| 5 | `neighbours` | run only with a kind and at least 2 (`SIMILAR_SHARED_MIN`) own links: those resolved by core `resolve_files` with the new file in the corpus, each named by its first end's canonical ID, else that end's path; nodes of the same kind (strings compared) with front-matter edges, not `mentions`, to at least 2 of them, a target holding more than 200 such corpus edges (`SIMILAR_TARGET_DEGREE_MAX`) skipped; the top 5 (`SIMILAR_NEIGHBOURS_MAX`) | shared count descending, path, node order |

The list is the five in that order; a node or proposal an earlier probe listed is not listed again, keeping its first source and the first new node it matched in text order. A search entry's rank is its true place in probe 4's order, counting nodes listed before, so the first may read rank 2. Gating entries past 64 are not listed. No order is by rowid: one index and queue state gives one list, the dry run's equal to the real call's, and copies at other roots and `HOME`s byte-equal.

## The entry and the notes

```json
{"id": "MEC-STAMINA", "path": "docs/spec/movement/stamina.md", "title": "Stamina", "kind": "mechanic",
 "source": "title", "reason": "title equal to new `MEC-ENDURANCE`'s", "gates": true}
```

`id` the node's canonical ID (`null` for an ID-less document) or the proposal's `PR-`; `path` its file, or the proposal's `target_path`; `title`, `kind` as indexed or parsed, else `null`. `reason`, one line, `<N>` the new node's name, lists at most 8 names (`SIMILAR_REASON_NAMES_MAX`), then `, and <k> more`:
- title: ``title equal to new `<N>`'s``; alias: ``alias `<A>` equal to new `<N>`'s title``;
- queue: ``` `PR-0001` (open), a live create, adds `<M>` titled as new `<N>` ```, the kind core's word, `<M>` the proposal's node ID, else its path;
- search: ``search rank <r> on `<t1>`, `<t2>` ``, the terms kept; neighbours: ``same kind `<K>`, <k> link targets shared: `<T1>`, `<T2>` ``.

Not stored, the note opens `notes` and is the text's head line; a dry run keeps `dry run: nothing stored` first and puts it second:

```text
not stored: a similar entry gates; propose an update of it, or name each gating entry in `distinct_from` (--distinct-from) to store it anyway
not stored: <n> similar entries gate and `distinct_from` names at most 64, so this create cannot be stored (a known limit)
```

## Document and text

`ProposalDocument`, and the daemon's `Proposal` (51 keys), hold `similar` after `why`, before `notes`: a list on a create's propose answer, stored, not stored or rehearsed (`[]` when nothing is found); `null` on every other kind, on a refusal, in `spec review`, `get_proposal`, approve and reject, and on every daemon route. Not stored has the rehearsal's shape (`proposals.rs` `rehearsed`: `id`, `status`, `base_commit`, `preview`, `revisions`, the times `null`; `diagnostics` as they would be stored). `distinct_from` stays `[]`.

Text (`--brief`, MCP content too): the head line, `introduced: <n>`, the finding lines, then per entry `similar: <id or path> | <source> | gates|advises | <kind or -> | <path> | <title or -> | <reason>`, controls escaped; none for an empty list. `spec review` prints no `similar` line.

## MCP, hub, plugin

`propose_change` (stdio `ChangeArgs`, hub `HubChangeArgs` through `From`) takes `distinct_from` and `keywords`, nullable arrays of strings: "create only: each gating similar entry you judge distinct, by its id, else its path, byte for byte." and "create only: extra search words, one word each." Either one non-null (`[]` too) on another kind: -32602 ``distinct_from: only a create takes it, not an update`` (core's kind word), after `dry_run`'s check, `distinct_from` before `keywords`, nothing stored. A broken cap: the CLI's line. The output schema's `similar` is `null` or items `{id, path, title, kind, source, reason, gates}`, `source` an enum of the five. `CHANGE_DESCRIPTION` 2 041 B and `HUB_CHANGE_DESCRIPTION` 2 037 name the gate; `INSTRUCTIONS`, stdio and hub, are unchanged.

A hub compares within its own store, its checkout's index and its queue; a terminal's rows are not on it. An older `specengine-mcp`, or a hub before 0.42.0, refuses both keys (`unknown field`). `propose-spec-change` and `write-flow` teach it (`plugin.md` "Skills").

## Measured

`specengine-eval scale` (`scale-eval.md`) times `SpecIndex::similar` on the warm index for the first 3 W targets (`reads.similar`, `growth.similar`, `calls` by target ID), each query the target's title upper-cased, its rare word a keyword, kind `node` and its document's `depends_on`. `similar_recall {planted, found, variants {exact, reordered, dropped, keyword}}`, each `{planted, found}`, after `hub`: 100 collisions, 25 a variant, on distinct live sections titled in 3 or more words, drawn after every other sample, so trees and digests are unchanged: `exact` the title upper-cased, `reordered` its words reversed, `dropped` its last word dropped, `keyword` two common words absent from it, the section's rare word a keyword; found when the section is among the rows. stderr adds `similar <ms>` and `similar recall F of P`. `scale_cli.rs` asserts the keys, 25 a variant and `exact` 25 of 25 at 1k, never a time or the overall recall.

Release, seed 1, 2026-11-12: medians 0.63, 3.79 and 9.54 ms at 1k, 10k, 100k (100k's max 15.0, growth 5.99 then 2.52), within RM-0059's 300 ms bar; recall 88, 80 and 72 of 100, at 10k exact 25, reordered 24, dropped 20, keyword 11 of 25 each, over the 70 % that would open the vector question.

## Known limits

- **The race**: the queue probe reads outside the write lock, so two creates of one title in the same instant may both store; the reservation's own read stays inside it (`proposal-kinds.md` "Reservation").
- **Case folding**: FTS5's trigram tokenizer folds case unlike Rust's `to_lowercase` (`İ`), so an exact twin may not be a title candidate; a title with no 3-character term is scanned, folded alike.
- **The text line**: a ` | ` inside a title shifts the fields; only `reason`, the last, is free. The JSON is exact.
- **U+0000** splits search terms too: a title holding one searches its parts; the title compare stays whole. MCP `search`'s own query still quotes a NUL term as given (earlier, not this probe's).
- **Keywords recall weakly**: 44 % of the keyword variant at 10k; the owner's call.
- **Unseen**: Tier 3 files, a terminal's queue from a hub, and another repository's row whose text this root's scheme parses otherwise.
- **An agent's own earlier create** (a cancelled call that stored) gates: name it in the report and ask for its reject, never name it distinct. Past 64 gating entries a create cannot be stored.

## Tests

Core, store, CLI, MCP, http `tests/intake_similar.rs`; store `similar.rs` units (`SimilarCaps` lowered); CLI `proposal_genre.rs` (`NEW_SOURCES`); MCP `plugin_skills.rs`, `plugin_files.rs`; http `daemon_keys.rs`, `spec_ids_hub.rs`; eval `scale_cli.rs`; UI `daemonKeys.test.ts`. Criteria: `docs/features/intake-similar.md`.
