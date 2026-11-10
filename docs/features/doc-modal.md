---
class: spec
status: shipped
shipped: 2026-11-11
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, fixtures]
ref: RM-0095 with RM-0091 and RM-0101; v5 step 3 (08 s2 Phase 7, "v5 scope"); Flow v2 step 3d and the owner's answers of 2026-11-10; Flow v2 Q3 and Q4 at their recommendations; DM-1 to DM-22
adrs: []
---

# doc-modal: one document view everywhere, a modal and a page, the path its breadcrumb (RM-0095)

## Why

The owner looked at his own project on 2026-11-10. In Flow's right panel a document came last, after kind, distance, `path:line`, inner counts and gap codes, in a column too narrow to read. He wanted one universal modal, like Jira's, holding all of a document's information; a minimal right side; nothing technical for people ("this is information for the AI"). Asked where, he answered "everywhere": Flow, the Spec tree, the Jump-to palette, links in cards and questions; a Cmd-click opens the document's own page, as for cards. Before: 36 call sites in 27 files linked into the Spec tree's technical node pane; links showed names, not titles; what a document holds showed only as tree rows; English slipped at 1 (RM-0091); Gaps read the whole tree to title a folded member (RM-0101).

How it works now: `docs/canon/doc-view.md`; Flow's panel `spec-flow-screen.md` "Panel"; the served keys `spec-cli-read.md` "Output", `spec-flow.md` "Output". No ADR: no rule of `architecture.md` changed.

## Description and interactions

Each decision is canon now; the numbers stay because the roadmap items cite them.

- **D1** One view, a modal and a page; the tree's node view stays technical (Flow v2 Q3): `doc-view.md` "The view". **D2** The `doc` pair on every route (Q4): "The hash". **D3** The card modal's manners, always the top dialog: "History and focus".
- **D4** Link ends' `title`, `end_kind`: `spec-cli-read.md` "Output". **D5** Gap `titles` (RM-0101): `spec-flow.md` "Output", `spec-flow-bar.md` "Gaps"; one MINOR, 0.34.0, for both.
- **D6** `DocLink` and the tree allowlist: "DocLink". **D7** The sections, **D8** Inside on expansion, **D11** the lazy body: "The view", "Inside". **D9** Flow's minimal panel: `spec-flow-screen.md` "Panel". **D10** RM-0091: `ui-languages.md` "Dates and counts". **D12** Link types raw: "Rules".

## Data

Keys: a link `{type, origin, at, name, title, end_kind, written, path, line, state, reason}`, then `kind`, `when` when labelled; a gap row `{code, state, count, items, titles, cause}`. `fixtures/daemon-keys.json`: 115 sets, `ShownLink` and `FlowGap` changed. MCP mirror: `ShowLink.title`, `.end_kind` (`Option<String>`), `FlowGap.titles` (`Vec<Vec<Option<String>>>`), required, nullable. UI types: `ui/src/api/provisional.ts`. On `fixtures/flow-aside`, `spec show POT-BISQUE --links --json`'s first outgoing and first incoming link, and `spec flow ATL-POTTERY --depth 1 --in-flow --json`'s `two-starts` row:

```json
{"type": "follows", "origin": "frontmatter", "at": null, "name": "POT-DRY", "title": "Drying on the racks", "end_kind": "potstep", "written": "POT-DRY", "path": "docs/workshop/run/bisque.md", "line": 10, "state": "resolved", "reason": null}
{"type": "depends_on", "origin": "frontmatter", "at": null, "name": "PUZZLE-0003", "title": "A puzzle of a cone that bends late, the first", "end_kind": "puzzle", "written": "POT-BISQUE", "path": "docs/puzzles/PUZZLE-0003.md", "line": 9, "state": "resolved", "reason": null}
{"code": "two-starts", "state": "measured", "count": 1, "items": [["POT-WEDGE", "docs/workshop/gallery/open-day.md"]], "titles": [["Wedging", "Open day"]], "cause": "more than one start: narrow [[flow.start]] to one"}
```

RM-0091's thirteen `_one` forms, now singular at 1 ("1 link not shown"): `board.view.omitted`, `tree.links.omitted`, `tree.binding.tokens`, `graph.holders`, `tree.node.holders`, `graph.details.moreText`, `tree.view.cut`, `tree.commands.verifyOnly`, `graph.limit`, `flow.notDrawn`, `roadmap.notDrawn`, `workspace.flowsShown`, `health.findings.totalShown`.

## Acceptance criteria

A **[look]** and a **[measure]** are recorded under "Implementation"; every other criterion is a test that its named mutation turned red (9 Rust, 24 UI).

- [x] DM-1 — **Link ends titled.** CLI `tests/doc_modal.rs`, on scratch copies of `spec-a`, `spec-b`, `flow-aside`: for each node (`ATL-POTTERY` plus every parse node on `flow-aside`, whose `spec tree --json` without a ROOT lists none), `spec show <name> --links --json`: a `resolved` link's `title`, `end_kind` equal the first node of `spec show <link.name> --json`, every other link's are `null`; the keys in "Data" order in every link; the text byte-equal to 3d3180e's (`doc_modal_before.json`, 222 reads); `POT-BISQUE` as "Data"; spec-b's dangling mention at `docs/spec/cli.md:25` both `null`. M: `title` from the shown node; `end_kind` from the edge label; `skip_serializing_if` on `title`; an incoming link titled by its target.
- [x] DM-2 — **Twins, keys, plugin.** MCP `get_node {POT-BISQUE, with: ["links"]}` equals the CLI's JSON and validates against `outputSchema` (`title`, `end_kind` required, nullable); HTTP `nodes/POT-BISQUE?with=links` byte-equal to the CLI; `spec health --json`'s amendment links carry both keys (checked by key order); `daemon_keys.rs`: 115 sets; `PINS`, `SCHEMA_PINS` end at 0.34.0; `spec_ids_hub.rs`, `tag-check.sh` read 0.34.0. M: the mirror without `end_kind`; `plugin.json` at 0.33.0.
- [x] DM-3 — **Gap titles (RM-0101).** `doc_modal.rs`: the `two-starts` row as "Data"; on `flow-gaps` every row's `titles` has `items`' shape and each title equals `spec show <member> --json`'s, `two-starts` `[["The first closed desk", "Ask at the desk", "The second closed desk"]]`; the text byte-equal; MCP `get_flow`'s schema requires `titles`; HTTP `/flow` byte-equal. Core `tests/flow_read.rs`: an untitled member serves `null`; 51 unreachable steps keep 51 items and titles in the core, and the CLI prints 50 of each. M: titles from the folded box (`"Firing run"`); `titles` not cut with `items`.
- [x] DM-4 — **Subset parity, cost.** The store differential stays byte-equal on every configured fixture with the new keys; `subset::seams` shows no whole decode for `show --links`. **[measure]** below. M: titles from a whole decode in the subset path.
- [x] DM-5 — **The hash.** `ui/src/app/routes.test.ts`: `doc` on `#/p?doc=A`, `#/p/board/C-1?doc=A`, `#/p/tree/R/page?doc=A`, `#/p/flow?scope=S&doc=A&stage=x` (`query` `scope=S&stage=x`); `#/p/flow?doc=A` no `query`; an empty or undecodable `doc` dropped, the first decodable wins; `#/p/tree/A?B` is `A`; `/page` only as `doc-view.md` says, `/pages` nothing; `withDoc`, `withoutDoc` round-trip Flow's `edge=`; `docPageHash(p, "ID#SEC")` encodes `#`. M: `doc` read only on Flow; `withDoc` keeping a second `doc`.
- [x] DM-6 — **Hash writes keep the modal.** `app/location.test.ts`, `FlowView.test.tsx`: Flow's bare-hash replacement, a seek's replace and a module drop keep `doc=A`; `pushHash` drops it; `closeDoc` after `openDoc` goes back, after a URL arrival replaces. M: `replaceHash` not carrying `doc`.
- [x] DM-7 — **`DocLink`.** `doc/DocLink.test.tsx`: `href` the page; a plain click pushes `withDoc` once, inside the modal replaces; Meta, Ctrl, Shift, Alt, `auxclick`, `contextmenu` prevent nothing and keep the hash; Enter acts as a click. M: preventing the default under `metaKey`.
- [x] DM-8 — **One link component.** `policy.test.ts`: the `sectionHash(…, "tree", …)` calls are exactly `doc-view.md` "DocLink"'s 16 in 13 files. `mocks/doc.smoke.test.tsx`, harbor-sim, from the Board tile, the card modal's target, Tasks' overview, the Inbox's proposal, Health's unbound row, Graph's list, the Questions round, the Roadmap lanes and Flow's Records: a plain click opens the modal on the document, the screen mounted beneath. M: `BoardCard.tsx` back on `sectionHash`; an allowlisted call dropped.
- [x] DM-9 — **The modal.** `doc/DocModal.test.tsx` on `docStub`: one `aria-modal` dialog labelled by its focused `h2` "The ferry", the view inert, live regions outside; from a link one push, Esc and Close go back, focus to the link; by URL, Esc replaces, focus to the view's `h1`; the scrim and Back close, Back with New card over it closes both; a link inside replaces `doc`; a 404's reason verbatim, `lang="en"`. M: no `closeOnScrim`; Esc always going back; focus left on `body`.
- [x] DM-10 — **Stacking.** `board/CardModal.test.tsx`: a target link pushes `#/<p>/board/<C>?doc=<T>`, the card modal inert beneath; Esc or Back closes only the document, focus to the target link; a cold render keeps the document on top. `FlowView.test.tsx`: a Records entry opens the modal over the dialog; Esc closes it, then the dialog. M: the document modal mounted before the view.
- [x] DM-11 — **The page.** `doc/DocPage.test.tsx`, `routes.test.ts`: `#/p/tree/DOM-FERRY/page` shows `h1` "The ferry", no tools, the nav on Spec tree; a remount keeps it; a crumb's plain click pushes its page; a 404 under `lang="en"`; `#/p/board/C-1/page` unchanged. M: `view: "page"` for `graph`.
- [x] DM-12 — **The sections.** `doc/DocBody.test.tsx` on `docStub`: crumbs Project, "The river", "The ferry" (current, no `href`); the in-work lozenge, `accepted` neutral, `lang="en"`; acts New card (the one primary), Open in Flow (`scope=DOM-RIVER`, `/DOM-FERRY`), Open in Spec tree, tools Open as a page and Close; no front-matter, no second title, both paragraphs and the section at reading width, `doc.throughAi`; Out "Boarding" (`uses` raw), Mentions (1) collapsed, the dangling `MEC-GANGWAY` only under Technical details; In "Landing" (`follows`); Cards `C-0007`, no New card; Technical details collapsed, holding the ID and copy, kind, `docs/ferry/ferry.md:1-<end>`, rev, tokens, both hashes, front-matter, the unresolved count, Open in Graph. M: front-matter in the body; links by `name`; Technical details open.
- [x] DM-13 — **Inside.** `doc/inside.test.ts`, `DocBody.test.tsx`: no `getTree` until expanded, then one `{root: "DOM-FERRY", depth: 2}`; "Directly under this document" (`mechanic`: Boarding), "Under Tides" (`assumption` 2, `question` 1); the ID-less line with 1; `ASM-0003` absent; `WBAY-KILN` (`aside`) `hunch` 50 then Show all 60, focus to the 51st; a second expansion reads nothing, the choice held on the next document; skeleton, failure with Retry, the empty line. M: grouping without the own-path test; kinds unsorted; the read on open.
- [x] DM-14 — **Flow's panel.** `FlowView.test.tsx` on `asideStub`: `POT-BISQUE` shows "Bisque firing", stage `bisque`, its work state, "Bone-dry pots are fired once, slowly, to a low cone."; acts Open document (primary) and New card, `KRUN-FIRING` adds "Open Firing run"; no `LinksPanel`, `CardsBlock`, Spec tree or Graph link; Technical details collapsed; no `getTrace({ref})` on selection; Open document pushes `withDoc` and draws from the held read. `markdown/Markdown.test.tsx`: the first paragraph skips front-matter, headings, a leading list, a table, a fence; none, nothing. M: the whole text in the panel; `getTrace({ref: N})` on a selection.
- [x] DM-15 — **Gaps from the read.** `FlowView.test.tsx` "the Gaps control (FS-17)", `ATL-POTTERY`: "Wedging + Open day", no `getTree`; a `null` title shows the name. M: `useTreeOnDemand` restored in Gaps.
- [x] DM-16 — **Spec tree and palette.** `TreeView.test.tsx`: the node pane's text, Links, Bundle, section links push `#/<p>/tree/<X>`, no modal; Open document pushes `?doc=<REF>`. `Palette.test.tsx`: a ref and a hit open the modal over the screen (`#/alpha/inbox?doc=<X>`), replace `doc` when open; search unchanged. M: the tree's `LinksPanel` on `DocLink`.
- [x] DM-17 — **RM-0091.** `i18n/plural.test.ts`: each "Data" key at 1 is the fixed form, at 2 unchanged; Russian untouched. M: `tree.links.omitted_one` restored.
- [x] DM-18 — **Both languages.** `catalog.test.ts` (parity, plurals, placeholders); `policy.test.ts` scans `src/doc/`; `screens.language.test.tsx` turns the modal and page Russian, titles and text verbatim, the daemon's words `lang="en"`. M: a literal "Links" in `DocLinks.tsx`.
- [x] DM-19 — **Tokens.** `styles/scale.test.ts`: 13 `SCALE_EXCEPTIONS`, none and no type `"RM-0095"`; `.md-url, .md-image-path` a type token; the doc CSS tokens alone; `IssueView.test.tsx` covers `meta`. M: `font-size: 15px` in `.doc-body`.
- [x] DM-20 — **[measure] Size.** First-load JS at most 845 kB; no chunk holding `DocBody` is the entry or a `modulepreload`.
- [x] DM-21 — **[look]** Light and dark, English and Russian, 1440x900, 1100x800, 320 wide, on the mock and a real game corpus: the modal over Flow, the Board and a card modal; the page; Flow's panel; Inside of the largest part within 1 s; long titles wrapping; one column at 320; focus rings, nothing cut unseen.
- [x] DM-22 — **Gates.** UI lint 0, `tsc` clean, the full run green twice; the Rust workspace run green; eval `doc_pointers`, `anonymity`, `ui_policy`; the docs check clean.

## Out of scope

Words for link types and kinds (Q-A, RM-0096); a served list of children; editing in the modal (ADR-0004); "Hand to your AI" in the view; the edge panel; Flow's seek for out-of-flow documents; the tree node view's layout (RM-0099); "Task" for a card (RM-0096). Working answers W1-W6 and the owner's Q-A to Q-C stand as `doc-view.md` describes: the page under the tree's route, the palette opening the modal, Inside read on expansion.

## Implementation

Reviews: ACCEPT, Rust and UI each in iteration 2; every departure accepted.

**Departures.** Rust: (1) the headless `no-start` row is titled by its first ROOT holder, `[[null]]` at Top; (2) members sort by (name, title); a `debug_assert!` on the shape; (3) the core keeps every title, the CLI cuts at 50; (4) control-character titles can pass `MAX_RESULT_CHARS`: residue, no code bound (`mcp-read.md` "Size"). UI: (1) the allowlist 16 calls in 13 files; (2) keys `tasks.overview.openDoc`, `inbox.card.openDoc`, `openDocId`, `palette.openRef`; added `doc.boundary`, `doc.notKnown`, `doc.technical.copyFailed`, `flow.factLabel`; removed `flow.openInTree`, `.openInGraph`, `.throughAi`, `.textHeading`, `.linksHeading`; (3) the edge panel lost `flow.throughAi`; (4) `useTreeOnDemand` stays for the seek; (5) Cmd-K over the modal, never another dialog; (6) `useDocNode` reuses another view's held read; (7) text headings one below "Text"; (8) `DocFrame` in first load, five slots lazy (`DocBody`, 11.22 kB, gzip 3.53); (9) types in `provisional.ts`, no generator; (10) a work item in a dialog: the body the one scroller, under 36rem the dialog; (11) the palette on a route with no modal pushes `withDoc(homeHash(project), ref)`; (12) openers passed explicitly; (13) Flow's panel notes a card made there until the selection changes.

**Churn beyond the plan**: HTTP `flow.rs`; `fixtures/trace-almanac/expected.json` (10 rows `"titles": []`); CLI `flow_authoring.rs`; MCP `mcp_flow_authoring.rs`, `mcp_size.rs` (ordinary titles, the residue pin), `plugin_files.rs` (`as_of_0_33_0`), `spec_ids_hub.rs`. `PINS` 0.34.0 `87e770c2…`, `SCHEMA_PINS` `fd816ac6…`, both set back red.

**Numbers.** DM-4, release, `scale` 10 000, four alternating runs, median p50 against 0.33.0: `get_node_links` 5 595 to 5 753 µs (+2.8 %), `get_flow_root` +0.5 %, others within 5.1 %, nondeterministic 0. DM-20: first-load JS 824.13 kB, gzip 236.00 (+9.00 on 815.13, bar 845); `ru` 144.26 kB, gzip 33.32. UI 4 862 tests in 163 files.

**Looks**, 2026-11-11, DevTools, a debug daemon on scratch copies. A real game corpus: the panel minimal; the modal 968 px, text 16/24 at 548 px, side 330, no overflow; Esc back, focus to the opener; the largest Inside 465 ms. The mock: a card's Source link stacks the modal over the card (inert), Esc closes the document alone; the palette over it replaces `doc`. `WBAY-KILN`'s Gaps: no tree read, no-start "Kiln bay". 320: no overflow; both themes, both languages, 1440 and 1100 fine.

| Module | What it does |
|---|---|
| core `flow.rs`; CLI `flow.rs` | `FlowGap.titles`; the JSON cut |
| CLI `links.rs`; MCP `mirror.rs`; `plugin/`, `.env.example` | link ends' `title`, `end_kind`; mirrors; 0.34.0 |
| UI `app/`, `doc/` | `doc`, `/page`, history, the modal, page, `DocLink`, Inside |
| UI `flow/`, `tree/`, `trace/`, `markdown/`, `ui/IssueView.tsx`, `api/` | the minimal panel, Gaps titles, `Facts`, `meta`, `useDocNode` |
| UI call sites, `palette/`, `i18n/`, `styles/`, `mocks/`, `test/` | `DocLink`, `doc.*`, RM-0091, tokens |
