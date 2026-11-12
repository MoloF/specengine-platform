---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-12
---

# The document view: one modal and page everywhere

A document opens as one view, a modal over any screen or its own page; the Spec tree's node view stays the technical one (`doc-modal`, code `ui/src/doc/`, `src/app/location.ts`). The layout: `ui-design.md` "The work item". Served keys: `spec-cli-read.md` "Output" (`ancestors`, a link's `title` and `end_kind`), `spec-flow.md` "Output" (a gap's `titles`), `spec-history.md` (`history`). No ADR: no rule of `architecture.md` changed.

## The hash

```text
#/<p>[/<section>[/<id>[/page]]][?<pair>[&<pair>]...]
doc=<ref>    the modal, on any project route; <ref> through encodeURIComponent
/page        after board/<C> (a card) and tree/<REF> (a document) only
```

- `parseHash` splits the first raw `?` on every route, so a raw `?` ends a Tree id (`sectionHash` writes none). `Route.doc`, on project and section routes: the first `doc` pair decoding (`decodeURIComponent`) to a non-empty string. Every `doc` pair leaves before Flow reads its pairs; `query` is absent when only `doc` stood. `view: "page"` for exactly `board/<C>/page` and `tree/<REF>/page`.
- `withDoc(hash, ref)` replaces every `doc` pair with one, appended after `?` or `&`; `withoutDoc(hash)` removes them, and a `?` left bare; `docOf(hash)`; `docPageHash(p, ref)` is `sectionHash(p, "tree", ref)` + `/page`.
- No modal on `#/` or a not-found route; the `#/` replacement to the first project's home drops `doc`.
- Mock examples: `#/harbor-sim/board/C-0003?doc=MEC-MOORING`, `#/harbor-sim/flow/MEC-MOORING?module=&scope=DOM-BERTHS&doc=MEC-MOORING`, `#/harbor-sim/tree/DOM-HARBOR/page`.

## History and focus

As the card modal's (`board-screen.md` "The card modal and page"):
- `replaceHash(hash, {keepDoc?})`, `keepDoc` true by default, carries an open `doc` pair to a hash naming none: a screen's own replace (Flow's bare hash, a seek, a correction, a module drop) never closes the modal. `pushHash` drops it.
- `openDocFrom(ref, opener, base?)` (`openDoc(ref)`: the focused element): a push of `withDoc` with no modal open, a replace with one, so a walk inside it (links, Inside, prose, a crumb, the palette) is one entry. `closeDoc()` (Esc, Close, the scrim): `history.back()` when this page pushed the entry, else `replaceHash(withoutDoc(…), {keepDoc: false})`. Back closes it; with New task over it, both. Openers are passed explicitly.
- Focus returns to the opener while it stands and is not inert, else to the view's `h1`, under a card modal to its title. Esc closes only the top dialog, never mid-IME; with a card write out nothing closes.
- **Stacking.** `DocModal` mounts in the Shell after the view: always the top dialog, over the card modal (a reload of `#/<p>/board/<C>?doc=<R>` too, the card landing later) and Flow's Records dialog; the view and every dialog beneath are inert.
- **Palette.** Cmd-K opens it over the modal when focus is inside the modal, never over another dialog. A ref or a hit opens the modal over the current screen (replacing `doc` when open); on a route that shows no modal, a push of `withDoc(homeHash(project), ref)`.

## DocLink

`DocLink({project, docRef, children, className?, tabIndex?, draggable?, ref?})` renders `<a href={docPageHash(project, docRef)}>`. A click with button 0, no Meta, Ctrl, Shift or Alt, not `defaultPrevented`, prevents the default and opens the modal, the anchor its opener; any other click, `auxclick` and `contextmenu` stay the browser's (a new tab opens the page). Every document link outside the Spec tree is one: Tasks, the Inbox, Board tiles and a card's targets, Roadmap lanes, Health, Graph's list, Questions, Flow's Records, `Markdown` outside the tree; `openDoc` serves the palette and Health's Enter.

**Spec tree links stay** where the tree is meant. `policy.test.ts` pins `sectionHash(…, "tree", …)` outside tests to 16 calls in 13 files: the tree's own navigation (`TextPanel`, `LinksPanel`, `TreeView` 2, `BundlePanel` 2); explicit acts (`RoadmapView`'s, `GraphDetails`') and the `o` key (`GraphView`; `FlowView` 2 with its lost-name link); `FlowRecords`' ID-less line; the tree branches of `Markdown` and `DriftPart`; the view's own Open in Spec tree (`DocBody`) and ID-less count (`DocInside`). Inside the tree, links keep moving the tree; `NodePane` adds Open document.

## The view

`DocModal` (`Dialog`, `aria-modal`, labelled by its focused `h2`, the title once read, the REF until then) and `DocPage` (`#/<p>/tree/<REF>/page`, an `h1`, no tools, the nav marking Spec tree) render `DocFrame`, an `IssueView`. The frame, its title and tools, is first load; its slots (`DocCrumbs`, `DocMeta`, `DocActs`, `DocMain`, `DocSide`) come from one lazy chunk, `DocBody.tsx`, never preloaded. The title node never swaps, so focus never leaves it. Width: `ui-design.md` "The work item".

- **Head.** Breadcrumbs: Project (`#/<p>`), each `ancestors` entry top first by title (else ID, else path), the document current; an ancestor's `href` is its page, its plain click in the modal replacing `doc`. Tools (modal): Open as a page, Close. `meta`: the work state, a `Lozenge` of the node's trace row (`rollupLook`), `doc.notKnown` with no row, none while reading; the raw `status:`, neutral, `lang="en"`, none for a section. Acts: New task (primary, `NewCardDialog` preset), Split to departments… (`card-split.md` "Screen"), Open in Flow (`flowHash`, `scope` the last ancestor, null at a root, `select` the node), Open in Spec tree; each a push, closing the modal.
- **Main.** A REF several holders answer: the first, and `doc.holders`. **Text**: the body without front-matter and without a first heading equal to the title (`{#…}` ignored), through `src/markdown/` at `--font-size-document` and `--reading-width`; its headings one level below the part heading "Text" (`h4` in the modal, `h3` on the page); anchors are `DocLink`s; then `doc.throughAi`. **Links**: Out, then In, the served strong links whose `state` is `resolved`, in served order, a row the raw type (neutral, `lang="en"`) then the end's `title`, else `name`, as a `DocLink`; `mentions` under a collapsed `Disclosure` with their count; none, `doc.links.none`. **Inside**, then **History**, below.
- **Side**, at most 360 px. Tasks: `CardsBlock`, its own New task off. Technical details, a collapsed `Disclosure` (`ui.technicalDetails`): the ID with a copy button (ID-less: the path; `doc.technical.copyFailed`); `Facts` (`src/tree/Facts.tsx`, shared with `NodePane`): kind, `path:line-end`, status, rev, tokens, span hash, flags; the norm hash; the front-matter verbatim; unresolved links, `<type> <written>`, state and reason, `lang="en"`; `left_out`; the read's notes; `doc.technical.inTree`; Open in Graph.
- **A 404**: the daemon's reason verbatim, `lang="en"`; only Close and Open in Spec tree stay. A section: its span, no status, Inside rooted at it.

## Inside

`insideOf(tree, directions): {groups: InsideGroup[], noId}` over `getTree({root: REF, depth: 2})`, read on the first expansion of the collapsed part, never on open:
- own nodes are the rows whose `path` is a depth-0 row's (the document and its sections); children, the other rows whose `parent` is an own node; deeper rows drop;
- groups: the document first (`doc.inside.direct`), then each section holding a child, in tree order (`doc.inside.under`, its title else ID), each a `section` one level below Inside's heading;
- within a group, `sets`: first the directions in served order, each a `role="group"` labelled by its title in a `bdi`, holding the children whose kind it is the first to list (`directions.md` "Inside"); then kinds no direction lists, by raw `kind`, code-point order, `null` last (`doc.inside.noKind`), each a `role="group"` labelled raw, `lang="en"`; entries in tree order, a `DocLink` by title else ID; a set shows `INSIDE_SHOWN` (50), then Show all, focus to the 51st. The directions come from the Shell's held workspaces read; pending or failed, raw kinds at once;
- ID-less children are counted, one line linking the Spec tree at REF;
- a skeleton while reading; a failure, the daemon's words and Retry; none, `doc.inside.none`.

The expanded choice is the Shell's per project for the page's life (`DocState {insideOpen, historyOpen}`; one choice leaves the other as it was).

## History part

`DocHistory.tsx`, a collapsed `Disclosure` after Inside, read as Inside is ("Reads"); the first holder's entries in served order, 18 `doc.history.*` keys:
- a skeleton; a failure, the daemon's words and Retry; `[]`, `none`; `truncated`, its line (alone when nothing is listed); the read's `history: ` notes, `lang="en"`, when `history` is `null` and beside a list;
- an `ol` entry says what changed (`history.ts`): `raised`, `applied` "Version N", `editorial` `wording`, `hand` `outside`, the three with `rev` `null` the raw word; `accepted` `task`, its ID a Board link; an item `question|discrepancy|decision` with its record a `DocLink`, none: `<kind>Plain`; an unknown word raw, `lang="en"`; `created` a `Lozenge` on any entry with `created`;
- then a `<time>`, `by` with `by_name` as served in a `bdi`; an apply's `proposedBy`, `proposer` `lang="en"`, the proposal an Inbox link (alone with no `proposer`); the rationale or title. No OID and no `by` is drawn.

## Reads

| Part | Call | When |
|---|---|---|
| Document | `getNode(REF, {with: ["links"]})` (`useDocNode`), its first node | on open; the subset read, about 56 ms at 100k |
| Work state, Cards | `getTrace({ref: REF})` | on open |
| Inside | `getTree({root: REF, depth: 2})` | on the first expansion, per document |
| History | `getNode(REF, {with: ["history"]})` (`useNodeHistory`), its first node | on the first expansion, per document |

The keys are `READS_OF_THE_SPEC`'s: an apply, a gap or a card event re-reads an open view; History's key is a node's, which no card event re-reads ("Known limits"). `useDocNode` refetches on mount only when no other view holds the read: Flow's box read is the same key, so Open document draws at once.

## Rules

- No kind, status, link type or rollup word is branched on but through `labels.ts`, and no history `change` word but through `history.ts`, the one table of change words (`SENTENCES`); `policy.test.ts` scans `src/doc/`. Link types stay raw (ADR-0031): words need a served label. No spec write (ADR-0004): `doc.throughAi`.
- Chrome is `doc.` keys (`flow.`, `tree.`, `ui.` where shown); the daemon's words verbatim, `lang="en"`. The CSS takes tokens only.

## Known limits

- Inside is the one O(N) read, about 1 s at 100k (`scale-eval.md` "Baseline", `get_tree`), a hub's project turn, paid per document while kept open; the largest part of a real game corpus read in 465 ms. Past 1 s the follow-up is a served list of children from the index's `PARENT` rows.
- Inside nests two deep: a child's children show in its own Inside. A flow's steps list in tree order, not walk order.
- Open in Flow on a document out of the flow acts as any undrawn name (`spec-flow-place.md` "The seek").
- A project named as its root document repeats in the breadcrumb (data). Prose wiki-style `[[path]]` references render raw.
- Widths, 320 px and stacking are proven by looks (2026-11-11), not tests: jsdom lays nothing out.
- `DocMeta` and `CardsBlock` find their own row by `node.id === docRef`, so a document opened by an alias shows its work state as not known (before RM-0100).
- An open History stays as read after a hand commit (no event names it) and after a task is accepted (card events re-read no node key), until the view is reopened.

## Tests

UI `src/doc/*.test.*` on `src/test/docStub.ts`, `app/{routes,location}.test.ts`, `mocks/doc.smoke.test.tsx` (nine screens), `mocks/history.smoke.test.tsx`, `api/history.live.test.tsx`, `board/CardModal.test.tsx` (stacking), `flow/FlowView.test.tsx`, `tree/TreeView.test.tsx`, `palette/Palette.test.tsx`, `ui/IssueView.test.tsx`, `i18n/plural.test.ts`, `policy.test.ts`; CLI `tests/doc_modal.rs` (served titles; the golden `doc_modal_before.json`).
