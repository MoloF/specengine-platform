---
class: spec
status: shipped
scope: [ui, crates/specengine-eval]
ref: ui-markdown analysis 2026-10-06; the owner's decision of 2026-10-06 (render markdown), every recommendation accepted; 08 s2 Phase 4
shipped: 2026-10-06
adrs: [ADR-0036]
---

# UI markdown: rendered prose

## Why

Node text, proposal prose and task plans showed as raw markdown (`ui-tree-node` deferred "a preview later with an ADR"); the owner reads documents, not markup (07 s3 Node: rendered, the source one click away). The owner decided 2026-10-06 to render markdown, every recommendation taken: `react-markdown` 10.1.0 + `remark-gfm` 4.0.1 (MIT, past `minimumReleaseAge`; runner-up `marked` + own renderer; rejected: an HTML string with a sanitizer, separate mdast packages, a hand-written parser, a daemon-built tree); external links not clickable; proposal and task prose rendered too; Rendered the default, Source a toggle away; ADR-0036 amends ADR-0033's allowlist (15 -> 17), never supersedes it. Rules now: `docs/canon/architecture.md#ui`, `ui/README.md` "Text". UI only, no daemon change.

## Acceptance criteria

Verifiers: Vitest (`ui/src/markdown/*.test.*` unless named), `policy.test.ts`, `ui_policy.rs`, the gates. M: the mutation turning it red; every one applied and red (18 in iteration 1, 11 in iteration 2).

- [x] AC-01 -- `package.json`, the README table, `ALLOWLIST`, `policy.test.ts` list the 15 plus `react-markdown` 10.1.0 and `remark-gfm` 4.0.1; the lockfile matches; a frozen install reports no unmet peer (`ui_policy.rs` `package_json_pins_exactly_the_allowlist_at_exact_versions`, `the_readme_table_names_the_allowlist_and_its_versions_once_filled`) (M: `^10.1.0`; an 18th package).
- [x] AC-02 -- only `src/markdown/` imports the two; no `rehype-raw`, `rehypeRaw`, `allowDangerousHtml` in `ui/src` (`only_src_markdown_imports_react_markdown_and_remark_gfm`, `no_raw_html_switch_in_ui_sources`, the ESLint rule) (M: `TextPanel.tsx` imports `react-markdown`).
- [x] AC-03 -- the hostile corpus (`<script>`, `<img onerror>`, `<iframe>`, `javascript:` and `data:` links, a raw `<a href>`, a footnote, an autolink): no `script`, `img`, `iframe`, `style`, `form`, `input` element, no `id` or `on*` attribute, every `a[href]` starts `#/`, the markup visible as text (M: the `a` override spreads its props).
- [x] AC-04 -- a link the stub resolves to `R-9` -> `#/alpha/tree/R-9`; a dangling one -> text with label and icon; before the read, no anchor; `[R-9](R-9)` anchored only when every entry at its place resolved alike (M: `href` from the destination).
- [x] AC-05 -- opening a node: one plain `getNode`, one with links; the Links tab adds none; the text renders before the links read resolves (`views.test.tsx`, `tree.smoke`) (M: the text waits for the links).
- [x] AC-06 -- an `https:` link: its text and URL, no anchor; `![a](p.png)`: "Image: a", no `img` (M: `<a target="_blank">`).
- [x] AC-07 -- `# T` gives no second `h1`; `## W {#RULE-X}` shows W with the label RULE-X, no `{#` text, no `id` (M: `{#RULE-X}` shown).
- [x] AC-08 -- Rendered: "Show in text" and "Sections" focus the block holding the line, front-matter lines counted; a mode switch after a jump leaves focus on the switch (M: lines counted from the body).
- [x] AC-09 -- Rendered is the default and kept across nodes; Source equals today's view of `text` (M: Source trims).
- [x] AC-10 -- front-matter a verbatim block, never an `hr` or a heading; an unclosed one renders as markdown (M: front-matter rendered).
- [x] AC-11 -- a table has `th` and a named focusable region, all 520 rows of `MEC-TIDE-TABLES`; task items an icon and a label; code keeps its whitespace (M: an enabled checkbox; `trim()`).
- [x] AC-12 -- the verbatim fields' `textContent` equals the string (M: `DiffView` through the renderer).
- [x] AC-13 -- an NFD heading keeps its code points; a 300-character token wraps in text, a cell and code; a tree keystroke re-parses nothing (`parse.test.tsx`) (M: `normalize("NFC")`; no memo).
- [x] AC-14 -- `pnpm lint`, `build`, `test` green, silent; `ui_policy` (23/23), `anonymity`, `doc_pointers` green (37/37); docs gate clean, worst W 108 261 <= 108 468 B; colours only in `tokens.css`, AA; `ui/README.md` 8 189 <= 8 192 B; ADR-0036 1 234 <= 1 536 B with "Cost" (M: a hex colour in `src/markdown/`).

## Owner's manual check

`pnpm --dir ui install --frozen-lockfile`, `pnpm --dir ui dev`; `http://127.0.0.1:5173/?scenario=normal#/harbor-sim/tree/DOM-BERTHS`: Rendered, front-matter verbatim, "the tide cycle" -> MEC-TIDES, "quay plans" Dangling; Source numbered; RULE-HARBOR-CLOCK: "One simulated clock" with its label, no braces; MEC-TIDE-TABLES: Tab to the table region, horizontal scroll, 520 rows; MEC-TIDES: Sections and "Show in text" focus the block; the Inbox's "Current section" switch; task plans render. With the daemon: resolved links anchored, raw HTML and external URLs as text.

## Out of scope

Syntax highlighting; images; external anchors; links on a bare `ID` or `ID#SECTION`; slugs; editing; any daemon change.

## Implementation

Two iterations; review accepted, iteration 2 fixed its seven findings.

| Module | What it does |
|---|---|
| `ui/src/markdown/Markdown.tsx` | the lazy renderer (`react-markdown`, `remark-gfm`, `remarkSpec`), parse memoised; overrides `a`, `img`, `h1`-`h6`, `table`, `input`, `code`, `pre`, none spreading props; an anchor only when every links entry at the link's path, line, `written` is `resolved` to one name |
| `ui/src/markdown/plugin.ts` | `remarkSpec`: comments, empty `<a id/name>` dropped, other raw HTML as text (`span.md-raw-inline`, `p.md-raw-html`, whitespace kept); heading shift, `{...}` label; images, footnotes, `data-line` |
| `ui/src/markdown/source.ts` | `splitFrontMatter` (as core's), core's destination scan for `written`; linear `withoutComments`, `attributeBlock`; `anchorTag` (core's `a_tag`) |
| `ui/src/markdown/Prose.tsx`, `TextMode.tsx` | lazy + Suspense, raw text while loading; Rendered/Source switch |
| `ui/src/tree/` | `NodePane`: plain and links reads in parallel, the jump cleared on a mode switch; `LinksPanel` takes that query; `TextPanel`, `TreeView` keep the mode |
| `ui/src/inbox/ProposalCard.tsx`, `ui/src/tasks/` | the prose fields rendered |
| `ui/src/mocks/` | `{#ID}` headings; `RULE-HARBOR-CLOCK`; DOM-BERTHS a resolved and a dangling link |
| `ui/eslint.config.js`, `Icon.tsx`, `app.css` | the import restriction; task-item icons; token-only styles |

Behaviour beyond the plan, accepted: the shallowest heading one below the view's own, cap 6; an autolink's URL once, mono; props `firstLine`, `path`, `target`, `onFollow`, `className` (`data-line` = file lines, front-matter counted); a link's line is where its destination starts (core's span); a document holder is `line === 1`; an image shows its decoded URL; a leading U+FEFF dropped when there is no front-matter; `<a name="x"/>` and `<a id="y" class="z"></a>` dropped as core reads them. Known: a heading may parse differently from core's `markdown.rs` in rare cases; Source settles it.

Packages: lockfile +97 (96 MIT, 1 ISC `@ungap/structured-clone`), no install scripts. Build: lazy chunk 165.77 kB (49.96 gzip); main 498.91 kB (145.47 gzip); the chunk's one `innerHTML` is the library's entity decoder (`ui_policy.rs` scans `ui/src`). Tests: Vitest 56 files, 1 110; new `Markdown`, `views`, `parse`, `source` tests (100k-input timing cases); amended `policy.test.ts`, `NodePane`, `TreeView`, `TasksView`, `tree.smoke`; `ui_policy.rs` ALLOWLIST 17, two new rules, ADR-0036 budget. `parse.test.tsx` flaked under load; `ui-health` made it deterministic (`act` + `waitFor`), 2026-10-07.

Shipped criteria changed by design (each spec has a line): `ui-tree-node` AC-01, AC-06, its "no markdown package"; `ui-tasks` AC-07.
