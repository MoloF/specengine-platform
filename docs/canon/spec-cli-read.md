---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-10-19
---

# spec search and spec show: output and the cap

What the agent read loop's two commands print, as text and JSON, and how the output cap cuts them; MCP `search` and `get_node` give the same bodies (`mcp-read.md`). Discovery, resolution, streams, the one-line rule, exits: `crates/specengine-cli/README.md`. Code: CLI `search.rs`, `show.rs`, `cap.rs`.

## Output

- `search`: per hit `<id or path> | <kind or -> | <title or -> | <path>:<line>` (+ ` | archived`), then the snippet on one line, four spaces in; then `hits 2 (limit 20); archived matches left out: 1 (--archive)` or `…; archive included`. JSON keys `archive, hits, kinds, limit, notes, query, tier3_left_out, truncated`; a hit `{id, kind, title, path, line, ord, archived, snippet}`, the snippet's line breaks raw.
- `show`: per node the search line's first four fields + ` | <tokens_est> tokens` (+ ` | status <s>` for a document, ` | rev <n>`, ` | archived`, ` | not UTF-8`, ` | norm <norm_hash>`) + ` | span <span_hash>`, then its bytes (a section's span, a document's file; `\n` added if missing), an empty line between nodes. JSON `{ref, reason, notes, nodes}`, a node `{id, kind, title, path, line, end_line, status, rev, tokens_est, archived, utf8, sections, norm_hash, span_hash, text, truncated, omitted, links, bindings}`, `span_hash` `b3:` of the whole span's bytes read (`propose --base`), `norm_hash` null or `node-revision.md`'s; `omitted` `{lines: [a, b], sections, sections_more, holders, holders_more}` when cut; `links` `null` without `--links` (`spec-cli-graph.md`).
- `show --bindings`: each node's bindings block (one line per binding row, then `  bindings <n>…`) after its links block, before its bytes; JSON `bindings` `{rows, omitted, tested}`, `null` without the flag or `[code]`; the code layer's causes are one `note: bindings:` line, never an exit (`code-reads.md`).

## The cap

`OUTPUT_CAP_CHARS` = 40 000 characters before the tail line (`View::Capped`; the daemon's `Browser` is uncut, `graph` too):

- `show`: on the text (JSON: the sum of `text`), cut at the last line end within it; a longer first line (a header included) at the cap. Tail `[truncated: <path> lines <a>-<b> not shown; sections not shown: <IDs or none>; holders not shown: <path:line, … or none>]`, a section not shown when its heading line is cut. Each list: the first `SHOW_TAIL_NAMES` (20; sections by source, holders by print order), then `, <k> more`. JSON drops the nodes after the cut one; the cut node's `sections` holds only the IDs whose heading line ends in its `text`, `omitted` the names by the same rule at the JSON's cut (`--links`, `--bindings`: the text's), `*_more` the k (0 if none). The bindings block counts with the links block, cut after a whole row; the tail then ends `; bindings not shown: <k>`.
- `search`: on the text, cut at a hit boundary, the same hits in text and JSON; a `note:`, `truncated: true`, tail `[truncated: <k> of <n> hits not shown; lower --limit or narrow the query]`; `hits <n>` counts every store hit. The first hit is always printed: alone over the cap, its snippet is kept first, then name (ID, else path), kind and title share the rest, shortest first; cut to nothing -> `-` in text, `null` in JSON.

## Search index

The store's rows under `search` (store `schema.rs`): `nodes_fts`, FTS5 over `id, title, own_text`, external content `nodes`, rowid `node_id` (never ordered by), `tokenize='trigram case_sensitive 0'`, kept by insert, delete, update triggers. `own_text` = the body minus its ID sections (core `own_spans`, the check's split too), joined by `\n`; next, `norm_hash` (`node-revision.md` "Where it appears"). The query and its order: `crates/specengine-store/README.md` "Writes, reads, search".

## Known limits

The read loop's open minors (the CLI README's "Open"):

- Note and tail say "title and snippet cut" for a cut name or kind too; a cut first hit's JSON `id` is marked by `truncated: true` alone.
- `one_line` flattens only CR, LF: VT, FF, NEL, U+2028/2029, ESC in quoted input reach stderr and `reason`.
- A caught parser panic prints Rust's panic message (fix: a quiet panic hook).
- `show`'s `cannot be read` exits are untested (a concrete `WorkingTree`; fix: `&dyn Source`).
- The reads decode every index row per call (`mcp-read.md` "Latency").
- bm25 statistics span the DB: ranks shift across worktrees; moved or deleted roots' rows stay (no prune).
