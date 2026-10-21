---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-10-21
---

# The lock diff panel: "Changes since the lock" in the Bindings tab

For each drifted binding of the tab's node that has a lock entry, the panel shows what moved on each side since the owner accepted it, as git's hunks against the working tree. Each row sits under verify's row line with its commands, so a drift item or an accept is decided without a terminal round trip. The read, its sides and its notes: `spec-lock-diff.md`; the tab around it: `drift-screens.md` "Bindings tab". Read-only, gating nothing (ADR-0053, ADR-0012, `architecture.md#control`); UI only, types provisional (ADR-0033). Built by `lock-diff-ui` (`docs/features/lock-diff-ui.md`, RM-0033).

## Layout

`tree/LockDiffPanel.tsx` `LockDiffPanel {project, nodeRef, onFollow?}` is mounted by `tree/BindingsPanel.tsx` once its read answered with a holder: after the holders, before "Commands for a terminal", whatever the rows say. `onFollow` is the tree view's, passed to each row's node link.

- `<section class="lock-diff">`, named by its disclosure: an h2 holding the button (chevron right or down, "Changes since the lock"), then the about line, shown collapsed or not; expanded, the body.
- Body, once answered (`.lock-diff-body`, `tabIndex=-1`, `aria-busy` while fetching): "Read again"; then `reason` alone, or the notes followed by each holder in the daemon's order; a later failure's note last.
- Holder (`<section class="lock-diff-holder" data-holder="<path>:<line>">`): with several (a REF declared twice), each is labelled "Changes at <path>:<line>" under an h3 of its place; with one, neither. Then its state or its rows.
- Rows: `counted(n, "drifted row", "drifted rows")`, then `health/codeRows.tsx` `CodeRowList` (10, then "Show all <n>"), in the daemon's order (verify's), never sorted or filtered. Each row is a `health/DriftPart.tsx` `DriftLine` (no Editorial mark) whose `children`, drawn last in its `<li>`, are the row's sides and then its commands.
- Sides (`.lock-diff-sides`, a grid of two `minmax(0, 1fr)` columns, one column inside `@media (max-width: 48em)`): spec, then code. Each side sent is a `.lock-diff-side[data-side=spec|code]` with a title line (`.lock-diff-side-title`, wrapped anywhere) and a body. A lone side spans both columns (`:only-child`; the base rule has `min-width: 0` and no `grid-column`). A `null` side draws nothing, since it did not move; both `null` (`marker_behind`, the node unmoved) leave the row line and its commands alone.
- A side's body: a non-empty `diff` goes to `ui/DiffView`, labelled by the title, hunks as sent (`pre-wrap`, uncut, no `max-height`); `""` and `null` are a paragraph ("States"), never a `DiffView`.

Styles: `styles/app.css` `.lock-diff*`, existing tokens only.

## Read

Collapsed at mount, nothing read. The first expanding enables `api/queries.ts` `useLockDiff(project, nodeRef, enabled)`: one `GET /api/projects/:p/lock-diff?ref=<REF>` (`client.getLockDiff`; a 404 is data, `reason` set). It stays enabled while the pane shows the node, under the key `queryKeys.lockDiff` `["lock-diff", p, ref]`: collapsing, expanding again or reselecting the tab reads nothing. `"lock-diff"` is in `READS_OF_THE_SPEC` (not `WALKS`), so `proposal.applied` and an event gap read it again, expanded or not; a panel never expanded is never read.

No event follows a code edit or `spec lock accept`. The panel's "Read again" sends one GET per click, the lock diff only ("Reading" meanwhile, `aria-disabled`, a click then ignored), keeps the last answer and announces "The changes since the lock of <REF> were read again."; the tab's own "Read again" reads the bindings only. GET only. Cost: about 116 ms at code-a plus 1 000 files (`spec-lock-diff.md` "Known limits"), in the project's turn.

## Texts

Every daemon word goes through `health/oneLine.ts` `oneLine` (the notes, a side's <why>, the titles' fields, the refused node); `reason` is drawn verbatim in `<p class="verbatim">`, as the tab's; hunks never. Apostrophes: text in the markup uses U+2019 (`&rsquo;`, the house convention), strings in attributes and props stay straight; this document writes them straight.

| Part | Exact text | Markup |
|---|---|---|
| disclosure | "Changes since the lock" | `<h2>` `<button aria-expanded aria-controls>` |
| about | "For each drifted binding with a lock entry, what moved in the node's text and in its code since the lock, as git's hunks against the working tree. Read when first opened; a code edit shows after Read again." | `.lock-diff-about` |
| re-read | "Read again", "Reading" meanwhile | `aria-describedby` the disclosure |
| notes | label "Notes from the daemon on the changes since the lock" | `overview/parts` `Notes` |
| rows | the count; list label "Changes since the lock of <ID or path>, in spec verify's order" | `CodeRowList` |
| spec side | "Node text since the lock: <path> at <commit>" | `.lock-diff-side-title`; the figure's label |
| code side | "Code since the lock: <file>, blob <blob>" | idem |
| row commands | list label "Commands for the row of <node>" | `CommandList` |
| refused | "No command for <node>: only a REF of Latin letters, digits and . _ / - is put in a command." | `<bdi class="mono">` the node |

## States

| Case | Shown | Markup |
|---|---|---|
| no answer, reading | "Loading the changes since the lock of <REF>" | `Skeleton`, 3 lines, in `aria-busy` |
| failed, no answer | "The changes since the lock of <REF> could not be read", the message, Retry | `ErrorPanel` |
| failed again | "The changes since the lock could not be read again: <message>" | `role="alert"`, the answer kept |
| `reason` set | the reason alone: no notes, holder or command | `<p class="verbatim">` |
| `rows` null (N0) | "Not judged: the daemon's note above says why." | `data-diff-state="not-judged"`, no count |
| `rows` `[]` | "None: no drifted binding of this node has a lock entry." | `data-diff-state="none"` |
| `diff` `""` | "No change since the lock" | `data-diff-state="equal"` |
| `diff` null, its note | "No diff: <why>" | `data-diff-state="not-shown"` |
| `diff` null, no note | "No diff: the daemon's notes above say why." | idem |

<why>: `tree/lockDiff.ts` `sideNoteOf(notes, row, side)`, the rest of the first note that starts `lock diff: <key>: no <spec|code> diff: `, where `<key>` is `rowKeyOf(row)`, verify's text key (N1-N5, N7 of `spec-lock-diff.md` "Notes"). Only text is matched; a `null` diff never reads "No change since the lock" and never draws a figure.

## Commands

Each kept row gets `tree/commands.ts` `nodeCommands(row.node)`, unchanged: `spec verify --node <ID>`, `spec propose drift <ID>` and `spec lock accept <ID>`, each with its meaning, the text as copied and a `CopyButton`. `tree/CommandList.tsx` `CommandList {commands, label}` draws them and also draws the tab's "Commands for a terminal". A `node` failing `NODE_ID` (`drift-screens.md` "Commands") gets the refusal and no command; an ID is never quoted. Nothing runs from the page; the terminal judges.

## a11y

- The disclosure: a button inside the h2 names the section; `aria-expanded`; `aria-controls` set only while expanded (as `health/FindingsRegion.tsx`); focus stays on the button through expanding, the figures' rendering and collapsing.
- Two "Read again" buttons share the tab, same words. The panel's has `aria-describedby` pointing at the disclosure's button, so a screen reader tells them apart; the tab's has none.
- Each hunk figure is named by its side's title (never "Section diff"); `DiffView` says "Added: " and "Removed: " per line (`.sr-only`).
- The body is `aria-busy` while fetching; after a successful Retry, focus goes to the body (`ui/useRetryFocus.ts`), as in the tab. With several holders, an h3 sits under the h2, so no level is skipped.

## Mocks

`mocks/lockDiff.ts`: harbor-sim's kept rows are Health's row 6 (`mocks/health.ts` `harborDrift`) matched by key, the key from `tree/lockDiff.ts` `rowKeyOf`. App code imports nothing from `src/mocks/`.

| Node, row | `spec` | `code` | Drawn |
|---|---|---|---|
| MEC-TIDES `conflict` | hunks | hunks | two figures, spec first |
| MEC-MOORING `code_ahead`, moved from `src/sim/berths.rs` | `null` | hunks | one figure, spanning |
| RULE-BERTH-DRAFT `marker_behind`, a gone commit | `diff` `null`, N1 | `null` | "No diff: <path> cannot be read at <commit>" |
| RULE-HIGH-WATER `symbol_gone` | `diff` `null`, a binary pair, N7 | `-` lines only | "No diff: git shows no hunk for bytes that differ", one figure |

ledger-api with no `[code]` (or `code-cannot`): `rows` null with the one `bindings:` note; `not-configured`: `[]`. No mock row has `diff` `""`; a stub view tests it.

## Known limits

- An expanded panel goes stale after a code edit or an accept until "Read again" or an apply. Each apply re-reads a panel expanded once, about 116 ms in the project's turn.
- A row key holding characters that verify's text escapes matches no note, so it gets the fallback, never "No change".
- Hunks are uncut, so a document node's can be long; a CRLF working copy differs on every line (`spec-lock-diff.md` "Known limits").
- No deep link, no `spec lock diff` row command, no word diff.
- The panel loads with the main chunk (480.13 kB, under Vite's 500 kB warning).

## Tests

`tree/LockDiffPanel.test.tsx` (stub views copying the mock: sides, hunks byte for byte, states, Read again, commands, figures, the disclosure, the CSS rules), `tree/lockDiff.test.ts` (`sideNoteOf`), `tree/NodePane.test.tsx` (the one read, the placement), `api/live.test.tsx` (an apply re-reads; never expanded, none), `mocks/lockDiff.test.ts` (the four rows), `mocks/tree.smoke.test.tsx` (the mock drawn, ledger-api), `policy.test.ts` (`ROW_SOURCES`: AC-U6, no sort). Not covered: the re-read on an event gap (only `proposal.applied` is tested), the body's `aria-busy` during a re-read, the real layout (jsdom matches only the CSS text).
