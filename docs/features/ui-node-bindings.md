---
class: spec
status: shipped
scope: [ui]
ref: RM-0011 slice 1 of 3, split 2026-10-21 (Q-A, Q-J, Q-M; owner pre-approved, review open); 07 s3 Node row; UI 2 iterations, review 1 accepted
shipped: 2026-10-21
adrs: [ADR-0046, ADR-0033]
---

# ui-node-bindings (RM-0011, 1 of 3): the node's Bindings tab

## Why

The daemon has served each node's code bindings since 7a-1 (`nodes/{*ref}?with=bindings`), but no screen asked for them: the owner saw drift only in Health's whole-tree row 6 and on a terminal. This slice adds a Bindings tab to the node pane: each holder's binding rows with their `sync`, its `[[tested]]` entries, and the node's terminal commands, which the owner copies and never runs from the page. Read-only and UI-only over an existing read; visibility, never a gate (ADR-0046, ADR-0012); types provisional (ADR-0033).

## Description and interactions

As built: `docs/canon/drift-screens.md` "Bindings tab" (the read, panel, row, states, commands, mocks). No deep link to the tab (Q-M).

## Data

- `api/client.ts` `NodeOptions.with?: ("links" | "bindings")[]`; key `queryKeys.node(p, ref, {with: ["bindings"]})` = `["node", p, ref, {with: ["bindings"], archive: false}]`, inside `["node", p]`.
- `NodeView {ref, reason, notes, nodes}`; `ShownNode` 19 keys, `bindings` the 19th after `links`: `ShownBindings {rows, omitted, tested} | null`, `BindingRow` 14 keys, `TestedRow` 7 (`docs/canon/code-reads.md` "spec show --bindings").
- Commands, `tree/commands.ts`: `NODE_ID = /^[A-Za-z0-9][A-Za-z0-9._/-]*$/`; `nodeCommands(ref: string, verifyOnly = false): NodeCommand[] | null`, verify alone when `verifyOnly` (several holders, or one holder whose `id` is null), `NodeCommand {action: "verify" | "drift" | "accept", meaning, text}`. Meanings:
  - verify: "Verify: judges the node's own bindings against spec.lock, as this tab does; a section by its own REF"
  - drift: "Queue its drift: judges the node's own rows at HEAD (commit edits first) and queues an Inbox item for your decision"
  - accept: "Accept: asks [y/N], then locks the node and its code at HEAD in a commit of spec.lock alone"
- Commands block texts: the note "Copy a command and run it in the project's worktree; only `spec lock accept` asks [y/N], before it writes spec.lock, and `spec propose drift` queues an item for your decision. Nothing runs from here, and nothing waits on these rows."; several holders "Only verify is offered: <REF> names <n> nodes, and `spec propose drift` and `spec lock accept` each name one."; an ID-less holder "<REF> names a document with no ID; `spec propose drift` and `spec lock accept` need one."; refused "No command for <REF>: only a REF of Latin letters, digits and . _ / - is put in a command."; the list "Commands for <REF>".
- Daemon keys: 74 to 76 sets, `ShownNode` (index 74, 19 keys, `bindings` after `links`) and `ShownBindings` (75: `rows, omitted, tested`), collected from code-health's `GOAL-01?with=bindings` (`health_code_repo`) and A's plain `MEC-STAMINA` (`bindings` null).
- Mocks: `mocks/bindings.ts`, as the canon's "Bindings tab" says.

## Rules and edge cases

- The panel SHALL send GET requests only, and only its one read.
- WHEN a holder's `bindings` is null, its part SHALL show no count or number: never 0, never "None" (M3).
- A row with `sync` null SHALL show "Not judged", never "In sync"; an unlisted word SHALL show raw, neutral.
- The UI SHALL NOT sort rows or entries, nor compare `sync`, `relation`, `kind`, `state` or `category` outside `labels.ts` (M2, M7).
- A command SHALL be fixed words plus the tab's REF matching `NODE_ID`, unquoted; otherwise there is none (M5). A REF naming several holders, or a document with no ID, SHALL get verify alone.
- Every word from the daemon SHALL show as text (no HTML sink), wrapped and uncut; `reason` verbatim, notes through `oneLine` as Health's causes.
- A look-alike or unresolved ID SHALL show as written, under the holder whose set holds it.

## Acceptance criteria

ui-developer; each mutation turned its test red once and was reverted.

- [x] UI-1: `tree/NodePane.test.tsx`. Tabs Text, Links, Bindings, Bundle, Proposals; opening Bindings sends exactly one `…/nodes/<REF>?with=bindings`, none before, none on reopening or another tab. M1 (the read moved to mount) red.
- [x] UI-2: per holder, rows in the daemon's order with every row field: `""` as "No ID", the three place forms, "No symbol", "Moved from", "Candidate"; several holders one part each; `mocks/tree.smoke.test.tsx` "normal: rows in the daemon's order". M2 (rows sorted by `sync` or by file) red. `NodePane.test.tsx` "the bundle's code rows" passes unedited.
- [x] UI-3: `null` shows "Not read: …" and the note, never a number; a `sync: null` row "Not judged"; zero rows "None: …". M3 (`null` drawn as "None") red.
- [x] UI-4: tested entries, `[]` and `null` each draw their own text. M4 (`null` drawn as `[]`) red.
- [x] UI-5: `tree/commands.test.ts`. `nodeCommands` gives the three commands, in order, for `LAW-BOUND`, `slug/AC-07` and a path REF; verify alone when asked (`slug/AC-07` for several holders, `docs/spec/b.md` for an ID-less document); null for `""`, a space, `;`, `$(`, a backtick, `"`, `'`, a newline (inside and trailing), a leading `-` or `.`, `#` and a Cyrillic look-alike (written escaped). M5 (a validator accepting a space) red, re-proved after iteration 2.
- [x] UI-6: GET requests only; `api/live.test.tsx`: `proposal.applied` re-reads the opened panel once, `proposal.created` nothing; "Read again" one GET per click. M6 (the key moved outside `["node", p]`) red.
- [x] UI-7: `policy.test.ts` AC-U6 scans `tree/BindingsPanel.tsx`, `tree/BindingLine.tsx`, `tree/commands.ts`; its field list gains `sync`. M7 (`row.sync === "ok"` in the panel) red.
- [x] UI-8: mocks as in "Data"; `mocks/corpus.test.ts` checks the rows' 14 keys, the entries' 7, in order, and the agreement with Health; the smoke covers `large` (Show all 3000) and `code-cannot`; `api/daemonKeys.test.ts` counts 76. `pnpm lint` 0, build clean, `pnpm test` 2 387 of 2 387; 17 packages; `tokens.css`, `tokens.test.ts` unchanged.

test-engineer:

- [x] TE-1: `crates/specengine-http/tests/daemon_keys.rs` and `fixtures/daemon-keys.json` hold `ShownNode` (19) and `ShownBindings` (3), 76 sets; `norm.rs` and `kind_amendment.rs` follow the count. M (`tested` dropped from the fixture's `ShownBindings`) red.
- [x] TE-2: M1-M7 each red, then reverted; eval `ui_policy`, `anonymity`, `doc_pointers` green; http `daemon_keys`, `norm`, `query`, `parity` 11/11; CLI `kind_amendment` 24/24.

spec-writer:

- [x] SW-1 (2026-10-21): this spec, slices 2-3 as drafts, RM-0011 split, RM-0033 to RM-0037, 08 s2.
- [x] SW-2 (2026-10-21): `docs/canon/drift-screens.md`; `code-reads.md` "Twins"; 76 sets in five canon files; `spec-cli-read.md` "Output"; 07 s3; 08 s2; RM-0011; `CLAUDE.md`; this spec compacted.

## Out of scope

Slices 2 and 3; RM-0033 to RM-0037; any write, stage, sort, filter, SSE trigger, deep link, `crates/*/src`, route, wire key, package or token.

## Implementation

| Module (`ui/src/`) | What it does |
|---|---|
| `tree/BindingsPanel.tsx` (new) | the tab: one read, Read again, states, notes, holders' rows and evidence, one commands block from the tab's REF |
| `tree/BindingLine.tsx` (new) | one binding row (`placeOf`, `bindingOf`), lifted from `BundlePanel.tsx`, which now renders it with `tokens` |
| `tree/commands.ts` (new) | `NODE_ID`, `nodeCommands(ref, verifyOnly = false)` |
| `tree/NodePane.tsx` | `TabId` `"bindings"`, the panel inside `opened.has` |
| `api/queries.ts` | `useNodeBindings` = `useNode(p, ref, {with: ["bindings"]})` |
| `api/client.ts`, `provisional.ts`, `app/sections.ts` | `with` widened; comments; the Tree's about line |
| `health/DriftPart.tsx` | `TestedLine` exported, `onFollow` optional |
| `mocks/bindings.ts` (new), `corpus.ts`, `MockClient.ts`, `health.ts` | `mockCode`, `bindingsOf`, rows shared with Health's row 6 |
| `styles/app.css` | the panel and command lines, existing tokens |
| Rust tests, fixture | `daemon_keys.rs`, `norm.rs`, `kind_amendment.rs`, `fixtures/daemon-keys.json` |

Differences from the draft, accepted in review 1 and iteration 2:

1. `TestedLine`'s `onFollow` is optional (Health passes none).
2. Notes go through `oneLine`, as Health's causes; `reason` stays verbatim.
3. Bundle's `""` node is a plain `-`, not mono; the node sits in `<bdi>`; the `binding-line` wrap rule now applies in Bundle.
4. `DriftPart` and `codeRows` load eagerly: the main chunk is 465.44 kB, under Vite's 500 kB.
5. Commands come from the tab's REF, not `ShownNode.id` (bare for feature-scoped sections); one block per tab; several holders or an ID-less document get verify alone, with a note. "No command: this node has no ID to name." is gone.
6. The state paragraphs carry `data-evidence-state`, not `data-tested`; `mockCode` runs only when bindings are asked.

Gates: `pnpm lint` 0, build clean, `pnpm test` 2 387/2 387; eval `ui_policy`, `anonymity`, `doc_pointers` green; http `daemon_keys`/`norm`/`query`/`parity` 11/11; CLI `kind_amendment` 24/24; M1-M7 red. Workspace: nextest 3 848 of 3 848 (63 skipped); fmt, clippy (probes too), export, check, parity and the eval policy tests (86 of 86) clean.

Known limit: `BindingLine` draws file, symbol, signature, category and detail raw, where Health's `DriftLine` uses `oneLine`; slice 2's shared row component settles it.
