---
class: spec
status: draft
scope: [ui, crates/specengine-eval]
ref: ui-markdown analysis 2026-10-06; the owner's decision of 2026-10-06 (render markdown), every recommendation accepted; 08 s2 Phase 4
adrs: [ADR-0036]
---

# UI markdown: rendered prose

## Why

Node text, proposal prose and task plans show as raw markdown: `ui/src/tree/TextView.tsx` prints the source unparsed, and `ui-tree-node` deferred "a preview later with an ADR". The owner reads documents, not markup (07 s3 Node: rendered, the source one click away). The owner decided 2026-10-06 to render markdown; ADR-0036 adds `react-markdown` and `remark-gfm`, amending ADR-0033's allowlist (15 -> 17). Rules: `docs/canon/architecture.md#ui`, `ui/README.md` "Text". UI only, no daemon change: it may run beside the Rust slices, but lands one after the other with `ui-health` (both edit `ui/README.md`).

**Owner's questions, decided as recommended**: react-markdown + remark-gfm (runner-up `marked` + own renderer; rejected: an HTML string with a sanitizer, separate mdast packages, a hand-written parser, a daemon-built tree); external links not clickable (URL as text); proposal and task prose rendered too; the Source toggle kept, Rendered the default; ADR-0036 amends ADR-0033, never supersedes it (`ui_policy.rs` reads ADR-0033 by name).

**Versions**, checked 2026-10-06 (`pnpm view <name> time`, from `ui/`): `react-markdown` 10.1.0, published 2025-03-07, the latest, MIT; `remark-gfm` 4.0.1, 2025-02-10, the latest, MIT; both past `minimumReleaseAge`. A newer version is an owner decision.

**Assumptions**: A1 the lockfile grows by about 90-100 packages (one ISC, `@ungap/structured-clone`), the lazy chunk by 40-60 KB gzip: estimates, the slice records the real numbers in "Implementation"; A2 the daemon's links read gives `written` as the destination's bytes with `<>` dropped; A3 one more `nodes?with=links` read per node opened, the text never waiting for it.

## Description and interactions

**Rendered**: the Node Text tab, per holder, with a Rendered/Source toggle (Source = today's numbered view); the inbox card's "Current section", same toggle; a task's plan, goal, criterion, assumption, owner note, run summary; a proposal's working answer, price of the other answer, rationale, an option's effect and price, the decision note.

**Verbatim, as today**: diffs, conflicts, evidence, the bundle body, search snippets, JSON, titles, row summaries, headings in lists, IDs, paths, kinds, statuses, reasons, notes in lists, errors, front-matter, code.

**Rendering rules** (`ui/src/markdown/`, the only importer of the two packages):

- **Local link** `[t](dest)`: an anchor only when the node's links read (the Links tab's query key, `getNode(p, ref, {with: ["links"]})`) lists an outgoing `inline` link at the same `path`, `line` and `written`, state `resolved`: `href = sectionHash(p, "tree", link.name)`. Any other state: text with the Links tab's state label and icon. Before the read lands, or outside the Node tab: text. No path joining, `link_base`, decoding or slugging in the UI (`docs/canon/spec-check-links.md` "File links").
- `ID`, `ID#SECTION` in prose: text. External links, autolinks, `mailto:`: the link text, then the URL in mono, no anchor. An image: ``Image: <alt>`` and its path, no `img`.
- **Headings** shifted below the pane's own (one `h1` per view), capped at 6; a trailing `{...}` hidden, its `#ID` shown as a label; no `id` attribute from data (hash routes, DOM clobbering); "Show in text" and "Sections" jump by `data-line`.
- **Raw HTML** never interpreted: comments and empty `<a id|name>` dropped, the rest literal text (not `skipHtml`). Footnotes: no generated heading, `id` or in-page `href`.
- **Tables**: `th`, wrapping cells, a focusable named scroll region. **Task items**: an icon and "Done"/"Not done", no `input`. **Code**: `pre > code` verbatim, wrapping, the info string as a label.
- **Front-matter** of a document holder: a verbatim block; an unclosed one is body text.
- No Unicode normalisation; one parse per text change (memoised).

## Data

`ui/package.json` `dependencies`: `"react-markdown": "10.1.0"`, `"remark-gfm": "4.0.1"`; `pnpm-lock.yaml` regenerated, installed frozen, no unmet peer (React `>=18`, we have 19.3.0), no install scripts. Types through `react-markdown`'s exports (no direct `@types/hast`).

`ui/src/markdown/Markdown.tsx` (lazy, as the canvas): props `source: string`, `links: ShownLink[] | null` (`null` = not read yet or not the Node tab), `project: string`, `baseLevel: 1-5`, `frontMatter: boolean`. Every block carries `data-line` (1-based, front-matter lines counted). Components overridden: `a`, `img`, `h1`-`h6`, `table`, `input`, `code`, `pre`, `html`-derived nodes; nothing spreads incoming props onto the DOM.

The mock: one fixture holder gains a `## W {#RULE-X}` heading (mocks today write `## ID: Title`).

**`ui/README.md` at the install**: the two rows join the "Dependencies" table and the "plus ..." sentence goes (`spec-writer`, the same pipeline step as the install, so `ui_policy.rs` meets 17 everywhere at once).

**Policy and test changes** (`test-engineer` unless noted): `crates/specengine-eval/tests/ui_policy.rs` `ALLOWLIST` 17 (test names stop counting "fifteen"); only `ui/src/markdown/` imports `react-markdown` or `remark-gfm`; `rehype-raw`, `rehypeRaw`, `allowDangerousHtml` nowhere in `ui/src`; the HTML-sink scan stays. `ui-developer`: `ui/src/policy.test.ts` 17 names and `/src/markdown/` among the href-rule folders; `eslint.config.js` `no-restricted-imports` for the two outside `src/markdown/`; a Vitest over a hostile corpus (library anchors bypass the source scan).

**Shipped criteria this slice changes by design** (their specs stay as shipped; this list is the record): `ui-tree-node` AC-01 (opening a node: one plain `getNode` and now one with links), AC-06 (the text without numbers equals `text`: now in Source only) and its working answer "no markdown package"; `ui-tasks` AC-07 (goal, plan, criterion, note, summary `textContent` equal to the JSON string: now rendered; `TasksView.test.tsx`).

## Rules and edge cases

- WHEN a link's destination is not in the links read as `resolved` THEN the system SHALL render text, never an anchor.
- WHEN the source holds `<script>`, `<img onerror>`, `<iframe>`, a `javascript:` or `data:` link or a raw `<a href>` THEN it SHALL appear as visible text, never as an element.
- Headings may read differently from core's parser in rare cases (`specengine-core` `markdown.rs` options: tables, heading attributes, strikethrough, task lists; remark-gfm adds footnotes and bare-URL autolinks, knows no `{#...}`): Source settles it.
- On `large`, the links read may lag: text first, anchors when it lands.
- Colours only from `tokens.css`; AA contrast; status by label and icon.

## Acceptance criteria

- [ ] AC-01 -- `package.json`, the README table, `ALLOWLIST`, `policy.test.ts` list the 15 plus `react-markdown` 10.1.0 and `remark-gfm` 4.0.1; the lockfile matches; a frozen install reports no unmet peer (M: `^10.1.0`; an 18th package).
- [ ] AC-02 -- only `src/markdown/` imports the two; no `rehype-raw`, `rehypeRaw`, `allowDangerousHtml` in `ui/src` (M: `TextPanel.tsx` imports `react-markdown`).
- [ ] AC-03 -- the hostile corpus (`<script>`, `<img onerror>`, `<iframe>`, `javascript:` and `data:` links, a raw `<a href>`, a footnote, an autolink): no `script`, `img`, `iframe`, `style`, `form`, `input` element, no `id` or `on*` attribute, every `a[href]` starts `#/`, the markup visible as text (M: the `a` override spreads its props).
- [ ] AC-04 -- a link the stub resolves to `R-9` -> `#/alpha/tree/R-9`; a dangling one -> text with label and icon; before the read, no anchor (M: `href` from the destination).
- [ ] AC-05 -- opening a node: one plain `getNode`, one with links; the Links tab adds none; the text renders before the links read resolves (M: the text waits for the links).
- [ ] AC-06 -- an `https:` link: its text and URL, no anchor; `![a](p.png)`: "Image: a", no `img` (M: `<a target="_blank">`).
- [ ] AC-07 -- `# T` gives no second `h1`; `## W {#RULE-X}` shows W with the label RULE-X, no `{#` text, no `id` (M: `{#RULE-X}` shown).
- [ ] AC-08 -- Rendered: "Show in text" and "Sections" focus the block holding the line, front-matter lines counted (M: lines counted from the body).
- [ ] AC-09 -- Rendered is the default and kept across nodes; Source equals today's view of `text` (M: Source trims).
- [ ] AC-10 -- front-matter a verbatim block, never an `hr` or a heading; an unclosed one renders as markdown (M: front-matter rendered).
- [ ] AC-11 -- a table has `th` and a named focusable region, all 520 rows of `MEC-TIDE-TABLES`; task items an icon and a label; code keeps its whitespace (M: an enabled checkbox; `trim()`).
- [ ] AC-12 -- the verbatim fields' `textContent` equals the string (M: `DiffView` through the renderer).
- [ ] AC-13 -- an NFD heading keeps its code points; a 300-character token wraps in text, a cell and code; a tree keystroke re-parses nothing (M: `normalize("NFC")`; no memo).
- [ ] AC-14 -- the gates green, no console output; `ui_policy`, `anonymity`, `doc_pointers` green; colours only in `tokens.css`, AA; `ui/README.md` <= 8 192 B; ADR-0036 <= 1 536 B with "Cost" (M: a hex colour in `src/markdown/`).

## Out of scope

Syntax highlighting; images; external anchors; links on a bare `ID` or `ID#SECTION`; slugs; editing (CodeMirror, later); any daemon change.

## Implementation

Filled in after implementation.
