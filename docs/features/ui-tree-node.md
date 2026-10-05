---
class: spec
status: shipped
scope: [ui, crates/specengine-eval]
ref: ui-tree-node analysis 2026-10-06, every recommendation accepted; client contract aligned with the daemon-read analysis; iterations 1-2, review accepted
shipped: 2026-10-06
---

# UI tree and node: reading the spec on mocks

## Why

Slice 2 puts the documented reads (`spec tree`, `show --links`, `search`, `bundle`) on one screen over the mock (ADR-0033): the containment tree, a node by REF with its verbatim text, links, the bundle an agent gets and its queue items, search. `daemon-read` will serve these reads; the client contract is its shape, so leaving the mock changes only the bootstrap.

Working answers, 2026-10-06 (orchestrator, the owner's standing instruction): next after `ui-shell`; no markdown package (verbatim source, a preview later with an ADR); browser reads uncut; the Bundle tab in, on demand; read-only; no new ADR. What became true: `ui/README.md` "Screen rules", "Owner's manual steps"; the client contract below.

## Data

**Client** (`ui/src/api/client.ts`; option types there). Names = the MCP tools' (`docs/canon/mcp-read.md` "Tools") = the daemon's query names; absent omitted, arrays repeat the key, `archive` only `true`.

```ts
/** GET /api/projects/:p/tree */
getTree(project: string, options?: { root?: string; depth?: number; kinds?: string[]; archive?: boolean }): Promise<TreeView>;
/** GET /api/projects/:p/nodes/:ref */
getNode(project: string, ref: string, options?: { with?: "links"[]; archive?: boolean }): Promise<NodeView>;
/** GET /api/projects/:p/search */
search(project: string, options: { query: string; kinds?: string[]; limit?: number; archive?: boolean }): Promise<SearchResults>;
/** MISSING ENDPOINT GET /api/projects/:p/bundle (07 §3 lacks it; rust-developer, daemon-read) */
getBundle(project: string, options: { node_ids: string[]; budget?: number }): Promise<BundleView>;
```

`getNode("harbor-sim", "MEC-TIDES#RULE-TIDE-WINDOW", {with: ["links"], archive: true})` → `GET /api/projects/harbor-sim/nodes/MEC-TIDES%23RULE-TIDE-WINDOW?with=links&archive=true`. The plain node read sends no `archive` (the CLI refuses `show --archive` without `--links`): "Include archive" reaches the tree, search and Links. 200: the document; 404 of `tree`, `nodes`, `bundle`: the exit-1 document, resolved as data; other non-2xx: `ClientError` `{status, message}` verbatim (exit 2 → 503); no response: status 0.

**Provisional types** (`ui/src/api/provisional.ts`), keys exactly the cited JSON's, absent = `null`: `TreeView`, `TreeNode` (`docs/canon/spec-cli-graph.md` "spec tree"); `SearchResults`, `SearchHit`, `ShownNode.span_hash` (`crates/specengine-cli/README.md` "Output and the cap"); `BundleView`, `BundleItem`, `WorkingAnswer`, `TailEntry`, `BundleLayers` (`docs/canon/spec-cli-bundle.md` "Output", "Layers"). Defined here, adopted by `daemon-read` (D2): `Snippet {segments: {text: string, hit: boolean}[], cut_start: boolean, cut_end: boolean}`, replacing the store's `**` markers, which are never parsed.

**Query keys** (`ui/src/api/queries.ts`), every argument present (`null`, `[]`, `false`): `["tree", p, {root, depth, kinds, archive}]`; `["node", p, ref, {with, archive}]`; `["search", p, {query, kinds, limit, archive}]`; `["bundle", p, {node_ids, budget}]` (enabled once the tab opens, a submit refetches); `["inbox", p]`. The Inbox's target read and the tree's plain read of one REF share an entry (equal arguments); the Links and archive reads are their own. After a decision, success or 409, `useDecideProposal` invalidates the project's inbox, tree, node, search and bundle keys.

**Mock** (`ui/src/mocks/`): a tree five levels deep with nested sections; an ID-less document; an ID with two holders; an archived document; a dangling parent; a two-document cycle; `MEC-TIDE-TABLES`, over 40 000 characters, whole; links in four states and `mentions`; inbox targets by ID, by the ID-less document's path (PR-0047), by a second `target_ids` entry (PR-0044, DOM-BERTHS); hostile titles and links; non-Latin as `\u` escapes; `MEC-PILOTAGE`'s status `proposed` (was `review`). `search` as the store: terms of 3+ characters, case-folded substrings, no Unicode normalisation, ANDed; a shorter term dropped with the CLI's note, none left → `ClientError` 503 with the CLI's line. `getBundle` canned, default 2 000, under the minimum 503; a look-alike letter in a REF → 503 naming the Latin fix. Scenario `large`: `harbor-sim` with 3 108 generated nodes, depth 0–5, uncut.

## Rules and edge cases

- WHEN corpus or daemon text is shown THEN it is text: no HTML sink in `ui/src` (`dangerouslySetInnerHTML`, `innerHTML`, `outerHTML`, `insertAdjacentHTML`, `document.write`, `createContextualFragment`, `srcdoc`); every `href` from `sectionHash`.
- An inbox target (`target_ids`, else `target_id`) matches a node equal to its `id`, to `<stem>/<id>` (stem: `path`'s file name without `.md`; `docs/canon/architecture.md` "Spec layout in a project") or, ID-less, to its `path`. Information only (ADR-0012).
- Kinds, spec statuses, link types, marks shown raw; app code quotes none but `mentions` and the closed tables (link states, marks, forms, layer keys, directions).

## Acceptance criteria

Verifiers: Vitest, `ui_policy.rs` (`test-engineer`), the gates. M: the mutation that turns it red, each Vitest one applied and red; `ui_policy.rs`'s sink detector is unit-tested on every sink.

- [x] AC-01 — the four methods carry their comments, `getBundle`'s `MISSING ENDPOINT` (`client.ts?raw`); on a counting stub `#/harbor-sim/tree/MEC-TIDES` makes one `getTree`, one `getNode`, no `getBundle` (M: a view imports `src/mocks/` or calls `fetch`; the flag removed).
- [x] AC-02 — each new type cites an existing heading; per type a `satisfies Record<keyof T, true>` record equals the cited key list; `ShownNode` has `span_hash` (M: an extra `TreeNode` key; `span_hash` removed; a cited heading renamed).
- [x] AC-03 — pre-order rows, `aria-level` depth + 1, `aria-setsize`, `aria-posinset`, `aria-expanded` on parents only; the ID-less row named by path; mark, archived, status, count as text; the cycle nested by depth; a stub `truncated` tree: the notice, "Show as root" calls `getTree` with `root` (M: constant `aria-level`; a mark by colour only; parents by `parent`).
- [x] AC-04 — one `tabindex="0"` in the tree; each key acts; Enter one push, arrows none; nothing from the search field or with Ctrl, Meta, Alt; `?` lists the keys (M: a `document` listener; a push per arrow).
- [x] AC-05 — `…/tree/MEC-TIDES`, `…/MEC-TIDES%23RULE-TIDE-WINDOW`, `…/docs%2Fspec%2Ftides%2Ftide-cycle.md` open their node, row current, ancestors expanded; a followed link focuses the `h1`; Back returns; unknown REF: the reason verbatim; look-alike: the error verbatim (M: REF unencoded; a generic "Not found").
- [x] AC-06 — the text without numbers equals `text`; the first number is `line`; the 300-character row wraps; both holders shown; a stub cut node lists lines and sections as links (M: `text.trim()`; first holder only).
- [x] AC-07 — `ui_policy.rs` `no_html_sink_in_ui_sources`: no sink of Rules in `ui/src/**/*.ts(x)`; Vitest: hostile title, text, snippet render no `img`; every `a[href]` starts `#/` (M: the snippet via `dangerouslySetInnerHTML`; `href={link.written}`).
- [x] AC-08 — links outgoing then incoming in the document's order; only `resolved` anchored; `reason` verbatim; state label + icon; "as <written>" against the target's name outgoing, the landing node incoming; "Show in text" focuses the line (M: sorted by type; a dangling anchor).
- [x] AC-09 — count and Proposals tab list exactly the items matching by ID, path, second `target_ids` entry, linking to the Inbox; `getInbox` failing errs only the proposals region (M: `target_ids` ignored; the node pane blanked).
- [x] AC-10 — no `getBundle` before the tab opens, then one, then one per submit; under the minimum: message verbatim, typed value kept; non-digits or over 4 294 967 295: a field message, no call (M: read on node open; field reset on refusal).
- [x] AC-11 — one `search` per Enter, none per keystroke (and a keystroke re-renders no tree row), `archive` passed; hits in `<mark>`, cut ends `…`; the count in the polite region; Esc restores tree and row; Enter on "Back to tree" opens no hit (M: a call per keystroke; segments joined plain).
- [x] AC-12 — per region: `slow` skeleton, `aria-busy="true"`; `empty` meaning and next step (the tree's note verbatim); `error` verbatim, Retry calls again; one failing stub method leaves the rest (M: one boundary for the view; an inert Retry).
- [x] AC-13 — the Inbox's target read and the tree's plain read of one REF share an entry, the Links and archive reads are their own; toggling archive reads tree, open Links and search again; a decision (success, 409) reruns tree, node, search, bundle (M: a key without options; only the inbox invalidated).
- [x] AC-14 — no kind, mock spec status or link type but `mentions` quoted in `ui/src` outside mocks and tests (`policy.test.ts`); ARIA tabs; one `h1` with two holders (M: `status === "accepted"` styling; an `h1` per holder).
- [x] AC-15 — `pnpm lint` (0 warnings), `build`, `test` green, no `console.error`/`warn`; `ui_policy`, `anonymity`, `doc_pointers` green (33 tests); 15 packages; mock kinds, `large`'s too, in their set; non-Latin escaped; `ui/README.md` ≤ 8 192 B; docs gate clean, worst W ≤ start (107 750 ≤ 107 890 B) (M: a 16th package; a raw Cyrillic letter in a mock).

## Owner's manual check

`pnpm --dir ui dev`: `#/harbor-sim/tree` by keyboard alone, VoiceOver reading level, position, expanded state, marks; `MEC-TIDE-TABLES` whole; at 200 % the panes stack without horizontal scroll; `?scenario=large#/harbor-sim/tree`: a root expanded, 100 rows arrowed without lag; `?scenario=empty`, `error`, `slow`.

## Open

- **Endpoints** (`rust-developer`): `bundle`, the uncut browser view of `tree`, `nodes/{*ref}`, `search`, structured snippets (D2), 404 with the exit-1 document, inbox `target_ids` (D1): specified by `docs/features/daemon-read.md`, closed when it ships. Until D1, a second target matches nothing against the daemon.
- **Non-Latin search**: the trigram does not normalise; a composed query misses decomposed text (`rust-developer`).
- **Latency**: `get_tree` 593 ms on 3 000 nodes, a bundle about 2 s: search on submit, bundle on demand; virtualization if `large` lags at the owner's check.

## Implementation

The screen: `#/<p>/tree[/<REF>]`, REF one segment (`sectionHash` encodes, `parseHash` decodes once); the tree pane (search above it, "Include archive", the cut notice with "Show <name> as root", "Left out" naming only non-zero parts) and the node pane (`h1` the REF, an `h2` per further holder; tabs Text, Links, Bundle, Proposals, manual activation), stacked below `48em`. Keys: tree Up/Down, `k`/`j`, Right, Left, Home/End, Enter (one history entry; arrows none); tabs Left/Right, Home/End, Enter/Space; search Enter, Esc (after an opened hit: that node's row), hits Up/Down, `k`/`j`, Enter; `?` lists them; a plain click to another hash focuses the new `h1`. Inbox cards gain "Open in spec tree".

| Module | What it does |
|---|---|
| `ui/src/tree/` | `TreeView` (panes; `SearchField` holds the typed query, only a submit lifts it), `SpecTree` (memoised ARIA tree), `rows.ts` (parents from `depth`), `NodePane` (holders, facts, tabs, follow intent cleared on hash change), `TextView`, `TextPanel`, `LinksPanel`, `BundlePanel`, `ProposalsPanel`, `SearchResults` (keys on the listbox only), `matching.ts` (Rules), `labels.ts` |
| `ui/src/ui/`, `ui/src/app/` | `Tabs.tsx`, `useRetryFocus.ts`, `RegionBoundary.tsx`; `Shell.tsx`, `sections.ts` (tree built), `location.ts`, `shortcuts.tsx` (keys per section) |
| `ui/src/api/` | `client.ts` the four reads; `provisional.ts` the read types; `queries.ts` keys, invalidation; `provider.tsx` an optional `queryClient` for tests |
| `ui/src/mocks/` | `corpus.ts` (tree, show, links, search, bundle as the CLI), `harbor-sim/large.ts`, `build.ts`, `MockClient.ts`, `scenario.ts` (`large`), both projects' fixtures and `kinds.ts` (spec statuses) |
| `ui/src/inbox/`, `ui/src/styles/` | `ProposalCard.tsx` "Open in spec tree"; link-state, tree-mark, search-highlight roles, a global `[hidden] {display: none !important}` |

Tests: 498 Vitest tests in 25 files, new `api/client`, `api/queries`, `tree/TreeView`, `tree/TreeView.renders`, `tree/NodePane`, `tree/rows`, `tree/matching`, `mocks/corpus`, `mocks/tree.smoke`; `crates/specengine-eval/tests/ui_policy.rs::no_html_sink_in_ui_sources`. Review of iteration 1, fixed in 2: search keys on the whole region (Enter on "Back to tree" opened a hit); incoming "as" compared with the source; a follow intent left set by a same-hash or modified click; a keystroke re-rendering every row and the pane; the budget bound; `h3` under `h1` with one holder.
