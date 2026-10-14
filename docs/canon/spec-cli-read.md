---
class: canon
tier: 2
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-10-14
---

# spec search and spec show: output and the cap

What the agent read loop's two commands print, as text and JSON, and how the output cap cuts them; MCP `search` and `get_node` give the same bodies (`mcp-read.md`). Discovery, resolution, streams, the one-line rule, exits: `crates/specengine-cli/README.md`. Code: CLI `search.rs`, `show.rs`, `cap.rs`.

## Output

- `search`: per hit `<id or path> | <kind or -> | <title or -> | <path>:<line>` (+ ` | archived`), then the snippet on one line, four spaces in; then `hits 2 (limit 20); archived matches left out: 1 (--archive)` or `…; archive included`. JSON keys `archive, hits, kinds, limit, notes, query, tier3_left_out, truncated`; a hit `{id, kind, title, path, line, ord, archived, snippet}`, the snippet's line breaks raw.
- `show`: per node the search line's first four fields + ` | <tokens_est> tokens` (+ ` | status <s>` for a document, ` | rev <n>`, ` | archived`, ` | not UTF-8`, ` | norm <norm_hash>`) + ` | span <span_hash>`, then its bytes (a section's span, a document's file; `\n` added if missing), an empty line between nodes. JSON `{ref, reason, notes, nodes}`, a node `{id, kind, title, path, line, end_line, status, rev, tokens_est, archived, utf8, sections, norm_hash, span_hash, text, truncated, omitted, links}`, `span_hash` `b3:` of the whole span's bytes read (`propose --base`), `norm_hash` null or `node-revision.md`'s; `omitted` `{lines: [a, b], sections, sections_more, holders, holders_more}` when cut.

## The cap

`OUTPUT_CAP_CHARS` = 40 000 characters before the tail line (`View::Capped`; the daemon's `Browser` is uncut, `graph` too):

- `show`: on the text (JSON: the sum of `text`), cut at the last line end within it; a longer first line (a header included) at the cap. Tail `[truncated: <path> lines <a>-<b> not shown; sections not shown: <IDs or none>; holders not shown: <path:line, … or none>]`, a section not shown when its heading line is cut. Each list: the first `SHOW_TAIL_NAMES` (20; sections by source, holders by print order), then `, <k> more`. JSON drops the nodes after the cut one; the cut node's `sections` holds only the IDs whose heading line ends in its `text`, `omitted` the names by the same rule at the JSON's cut (`--links`: the text's), `*_more` the k (0 if none).
- `search`: on the text, cut at a hit boundary, the same hits in text and JSON; a `note:`, `truncated: true`, tail `[truncated: <k> of <n> hits not shown; lower --limit or narrow the query]`; `hits <n>` counts every store hit. The first hit is always printed: alone over the cap, its snippet is kept first, then name (ID, else path), kind and title share the rest, shortest first; cut to nothing -> `-` in text, `null` in JSON.
