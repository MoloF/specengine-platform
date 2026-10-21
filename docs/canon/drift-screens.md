---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-10-21
---

# Drift screens in the web UI

Drift shown where the owner already reads, in the browser: the node's Bindings tab ("Bindings tab"); a drift item's typed rows with Health's row details (`ui-drift-evidence`) and the tree's badges (`ui-tree-drift`) are RM-0011's next slices. Read-only over reads the daemon already serves: no route, key on the wire, write, stage or SSE trigger of their own. Visibility, never a gate (ADR-0046, ADR-0012, `architecture.md#control`); types provisional (ADR-0033, `ui/README.md` "Contract seam"). A state is the daemon's word shown through `tree/labels.ts` or `health/labels.ts`, never compared or sorted elsewhere (`policy.test.ts` AC-U6); every word is text, wrapped, uncut.

## Bindings tab

**Read.** The node pane's tabs: Text, Links, Bindings, Bundle, Proposals (07 s3; `tree/NodePane.tsx`). The tab's first opening sends one `GET /api/projects/:p/nodes/<REF>?with=bindings` (`api/queries.ts` `useNodeBindings` = `useNode(p, ref, {with: ["bindings"]})`): `spec show REF --bindings --json` in `View::Browser`, uncut (`code-reads.md` "spec show --bindings"); never on mount, never with `archive`. The panel mounts inside `opened.has("bindings")` and stays: reselecting the tab reads nothing. Its key `["node", p, ref, {with: ["bindings"], archive: false}]` sits inside `["node", p]`, so `proposal.applied` and an event gap re-read it (`READS_OF_THE_SPEC`). A code edit, `spec lock accept` or an evidence commit fires no event (`health-drift.md` "Known limits"): "Read again" sends one GET per click ("Reading" meanwhile, a click then ignored), keeps the last answer with `aria-busy` and announces success; re-entering the node reads afresh. Cost: about 51 ms at code-a plus 1 000 files (`code-reads.md` "Known limits"), in the project's turn (`serve.md` "Concurrency"), so on demand only.

**Panel** (`tree/BindingsPanel.tsx`), in order: an about line and "Read again"; `reason`, verbatim; the read's notes, each through `health/codeRows.tsx` `oneLine` as Health's causes, in the daemon's order (the `bindings:` and `tested:` causes, `code-reads.md` "Degrade"); each holder in the daemon's order; one commands block. One holder: h2 parts, no holder heading. Several (a REF declared twice): an h2 `<path>:<line>-<end_line>` each over a section "Bindings at <path>:<line>", h3 parts.

- **Code bindings**: `counted(n, "row", "rows")`, ", <k> more not shown" when `omitted` > 0 (never in the browser view); rows through `CodeRowList` (10, then "Show all <n>", focus on the 11th), each a `BindingLine`.
- **Test evidence**: every entry, `verified` too, a `health/DriftPart.tsx` `TestedLine` (`testedLook`), its node a tree link taking the tree view's `onFollow` (optional, as Health omits it).

**Row** (`tree/BindingLine.tsx`; the Bundle tab renders the same line plus `<n> tokens`): the node as sent in a `<bdi>`, never a link, `""` "No ID" here and a plain `-` in Bundle; relation raw; place `<file>:<line>-<end_line>`, `<file>:<line>` or `<file>`; binding `<kind> <symbol>[ <item_kind>]`, "No symbol" when `kind` or `symbol` is null; `layer <layer>` when set; a Sync badge (`syncLook`: null "Not judged", never "In sync"; an unlisted word raw, neutral); category and detail joined; "Signature:" and the text; "Moved from <file> | <binding>"; "Candidate <file> | <binding>" per candidate. The `binding-line` wrap rule holds in both tabs.

| Case | The panel shows |
|---|---|
| no answer yet | a skeleton, "Loading the bindings of <REF>" |
| failed, nothing shown | "The bindings of <REF> could not be read", the message, Retry |
| failed, an answer shown | "The bindings could not be read again: <message>" (`role="alert"`) |
| `reason` set | the reason; no holder, no command |
| a holder's `bindings` null | "Not read: no code binding of this node came with the read; the daemon's note above says why." No count or number, never 0 or "None" |
| `rows` `[]` | "None: no marker or lock entry binds code to this node or a section within it." |
| `tested` `[]` | "None: no `[[tested]]` block covers this node or a section within it." |
| `tested` null | "Not judged: the `[[tested]]` entries were not judged on this read; the daemon's note above says why." |

State paragraphs carry `data-evidence-state`.

**Commands** (`tree/commands.ts`): one block per tab, "Commands for a terminal", after the holders, when the read returned a node. Built from the REF the tab was opened with (the route's, as the daemon resolved it; `slug/ID` whole, an ID-less document its path), never a holder's bare `id`. `nodeCommands(ref, verifyOnly = false)` puts a REF matching `NODE_ID` `^[A-Za-z0-9][A-Za-z0-9._/-]*$`, unquoted, after fixed words: `spec verify --node <REF>`, `spec propose drift <REF>`, `spec lock accept <REF>`, each with its meaning and a `CopyButton`, offered whatever the rows say (the terminal judges). A REF naming several holders gets verify alone, with a note that the other two each name one node; so does one holder whose `id` is null: "<REF> names a document with no ID; `spec propose drift` and `spec lock accept` need one." Any other REF (a space, a quote, `;`, `$`, a backtick, `#`, a newline, a leading `-` or `.`, a non-Latin letter) gets "No command for <REF>: only a REF of Latin letters, digits and . _ / - is put in a command." The block's note: "Copy a command and run it in the project's worktree; only `spec lock accept` asks [y/N], before it writes spec.lock, and `spec propose drift` queues an item for your decision. Nothing runs from here, and nothing waits on these rows."

**Keys.** `fixtures/daemon-keys.json`, 76 sets, last `ShownNode` (19 keys, `bindings` after `links`) and `ShownBindings` (`rows, omitted, tested`), from code-health's `GOAL-01?with=bindings` and A's plain `MEC-STAMINA`. `ShownNode` is the only set holding `norm_hash`; `NodeView`'s top level never does (`crates/specengine-http/tests/norm.rs`).

**Mocks** (`mocks/bindings.ts`, `corpus.ts` `nodeViewOf`): `bindings` null unless asked (`mockCode` runs only then). harbor-sim's rows agree with Health's row 6 (its 12 non-`ok` rows, `counts.ok` rows `ok`); `large` adds 3 000 `unlocked` rows; `empty` sets `reason`. `code-cannot`: harbor-sim null with a `bindings:` note, ledger-api rows with `sync` and `tested` null. ledger-api otherwise null with the no-`[code]` note, but `not-configured`: its rows, all `ok`.

## Known limits

- `BindingLine` draws file, symbol, signature, category and detail raw, where Health's `DriftLine` puts them through `oneLine`; `ui-drift-evidence`'s shared row component settles it.
- No deep link to a tab; no trigger of its own: the tab is stale after a code edit until "Read again", re-entry or an apply.
- `DriftPart` and `codeRows` load with the main chunk (465.44 kB, under Vite's 500 kB warning).

## Tests

UI: `NodePane.test.tsx` (tab order, the one read, rows, states, commands), `commands.test.ts`, `tree.smoke.test.tsx`, `corpus.test.ts`, `live.test.tsx`, `policy.test.ts` (AC-U6), `daemonKeys.test.ts`, `http.test.ts`. Rust: http `daemon_keys.rs`, `norm.rs`; CLI `kind_amendment.rs` (the count).
