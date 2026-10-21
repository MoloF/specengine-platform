---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-10-21
---

# Drift screens in the web UI

Drift shown where the owner already reads, in the browser: the node's Bindings tab ("Bindings tab") and a drift item's stored rows on its Inbox card, drawn by Health's row 6 line ("Drift rows"); each tree line's own drift ("Tree badges"). Read-only over reads the daemon already serves: no route, key on the wire, write, stage or SSE trigger of their own. Visibility, never a gate (ADR-0046, ADR-0012, `architecture.md#control`); types provisional (ADR-0033, `ui/README.md` "Contract seam"). A state, class, category or sync is the daemon's word shown through `tree/labels.ts` or `health/labels.ts`, never compared or sorted elsewhere (`policy.test.ts` AC-U6 and the no-sort test scan these screens' sources); every word is text, wrapped, uncut.

## Bindings tab

**Read.** The node pane's tabs: Text, Links, Bindings, Bundle, Proposals (07 s3; `tree/NodePane.tsx`). The tab's first opening sends one `GET /api/projects/:p/nodes/<REF>?with=bindings` (`api/queries.ts` `useNodeBindings`): `spec show REF --bindings --json` in `View::Browser`, uncut (`code-reads.md` "spec show --bindings"); never on mount, never with `archive`. The panel mounts inside `opened.has("bindings")` and stays: reselecting the tab reads nothing. Its key `["node", p, ref, {with: ["bindings"], archive: false}]` sits inside `["node", p]`, so `proposal.applied` and an event gap re-read it (`READS_OF_THE_SPEC`). A code edit, `spec lock accept` or an evidence commit fires no event (`health-drift.md` "Known limits"): "Read again" sends one GET per click ("Reading" meanwhile, a click then ignored), keeps the last answer with `aria-busy` and announces success; re-entering the node reads afresh. Cost: about 51 ms at code-a plus 1 000 files (`code-reads.md` "Known limits"), in the project's turn (`serve.md` "Concurrency"), so on demand only.

**Panel** (`tree/BindingsPanel.tsx`), in order: an about line and "Read again"; `reason`, verbatim; the read's notes, each through `health/oneLine.ts` `oneLine` as Health's causes, in the daemon's order (the `bindings:` and `tested:` causes, `code-reads.md` "Degrade"); each holder in the daemon's order; one commands block. One holder: h2 parts, no holder heading. Several (a REF declared twice): an h2 `<path>:<line>-<end_line>` each over a section "Bindings at <path>:<line>", h3 parts.

- **Code bindings**: `counted(n, "row", "rows")`, ", <k> more not shown" when `omitted` > 0 (never in the browser view); rows through `CodeRowList` (10, then "Show all <n>", focus on the 11th), each a `BindingLine`.
- **Test evidence**: every entry, `verified` too, a `health/DriftPart.tsx` `TestedLine` (`testedLook`), its node a tree link taking the tree view's `onFollow` (optional, as Health omits it).

**Row** (`tree/BindingLine.tsx`; the Bundle tab renders the same line plus `<n> tokens`), every daemon word on one line through `oneLine` (`health/oneLine.ts`, a leaf over `ui/text.ts` `shownText`, re-exported by `codeRows.tsx`: a line break a space, other controls and bidi marks `\u{..}`): the node as sent in a `<bdi>`, never a link, `""` "No ID" here and a plain `-` in Bundle; relation raw; place `<file>:<line>-<end_line>`, `<file>:<line>` or `<file>`; binding `<kind> <symbol>[ <item_kind>]`, "No symbol" when `kind` or `symbol` is null; `layer <layer>` when set; a Sync badge (`syncLook`: null "Not judged", never "In sync"; an unlisted word through `oneLine`, neutral); category and detail joined; "Signature:" and the text; "Moved from <file> | <binding>"; "Candidate <file> | <binding>" per candidate. The `binding-line` wrap rule holds in both tabs.

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

**Keys.** `fixtures/daemon-keys.json`, 76 sets, last `ShownNode` (19 keys) and `ShownBindings` (`rows, omitted, tested`); only `ShownNode` holds `norm_hash` (`crates/specengine-http/tests/norm.rs`).

**Mocks** (`mocks/bindings.ts`): `bindings` null unless asked. harbor-sim's rows agree with Health's row 6; `large` adds 3 000 `unlocked` rows; `empty` sets `reason`; `code-cannot` a `bindings:` note (harbor-sim) or null `sync` and `tested` (ledger-api), whose rows are otherwise null but in `not-configured`.

## Drift rows

One row component draws `spec verify`'s row in both places: Health's row 6 (the working tree's, `health-drift.md` "UI") and a drift item's stored rows on its Inbox card (as of the item's `base_commit`). No text says "now".

**Line** (`health/DriftPart.tsx` `DriftLine {project, row: StoredDriftRow, markEditorial?}`): `<li class="health-left-row health-code-row" data-drift=<node> tabIndex=-1>`, every daemon word through `oneLine`, an unlisted class or state word too (`health/labels.ts` `rawClass`, `syncLook`; its badge neutral). In order:

- head: the Class badge (`driftClassLook`), the node linked to the tree (`""` "No ID"), the relation, "Pre-existing" when `pre_existing`, "Editorial" when `markEditorial` and `isEditorialRow(row)`;
- judgement: the State badge (`syncLook`), category and detail joined;
- place: the file, `<kind> <symbol>[ <item_kind>]` (`tree/BindingLine.tsx` `bindingOf`) or "No symbol";
- facts: "No marker", "Marker line 41 @5" or "Markers line 41 @5, line 88 unversioned" (a null `rev`: unversioned or malformed, the row's `bad_marker bad_rev` tells which); "Node rev 5" or "No node revision"; "Locked rev 5, <commit>, <verified_at>" (the whole commit, the date in a `<time>`) or "Not locked"; "Compared <word>" or "Not compared";
- levels, only when `levels` is an object: "Levels", "lock sig, body", "markers sig, body", "changed body" (none when `changed` is null), `[]` "none";
- tails: "Moved from <file> | <binding>", "Candidate <file> | <binding>" each. An absent `levels` or `anchor` (an older build's 15-key row) draws as null.

`health/labels.ts` `isEditorialRow({state, category})`, true for `spec_ahead` with `same_revision` alone, is the one predicate there: the words passed in, never `row.state ===`.

**Guard** (`inbox/drift.ts` `storedRowOf(row: unknown): StoredDriftRow | null`): a JSON object (not an array) whose keys, read by name from a list, fit verify's row (`spec-verify-output.md` "JSON"; the table: `docs/features/ui-drift-evidence.md` "Data"): `levels` and `anchor` may be absent, a count is a non-negative safe integer, later keys are ignored; anything else is null. `api/provisional.ts` `StoredDriftRow` is `DriftRow`'s 17 keys, `levels?` and `anchor?` optional; `DriftEvidence.rows: StoredDriftRow[]`. The daemon passes rows as stored, so no key set: `fixtures/daemon-keys.json` stays at 76.

**Card** (`inbox/DriftItem.tsx` `DriftSection {project, drift, commit}`): the about line, Rows (`driftRowsText`), Commit judged, Evidence hash; then `<ol class="drift-rows" aria-label="Stored rows, in spec verify's order">`, every stored row in stored order, never sorted, filtered or cut (at most 64): a typed one a `DriftLine` with `markEditorial`; else `<li class="drift-raw" data-raw="">` "Shown as sent: not in the shape this screen reads." over `<code>` `shownText` of its compact JSON (bidi marks, DEL and C1 as `\u{..}`), still counted. No row makes the card throw.

**Commands** (`inbox/drift.ts` `driftCommands`): `spec approve PR`, `spec approve PR --editorial`, `spec reject PR --reason "..."`, all three whatever the rows: they were judged at `base_commit`, and approve judges `HEAD` again (`proposal-drift.md` "Approve"). `--editorial` reads "Approve as editorial: only when the node's text changed at its locked revision and keeps its meaning; rows marked Editorial were so when the item was raised, and the terminal judges HEAD again; a plain approve refused for that names this command". A `conflict` at R = `rev_L` is never marked (that needs `node_rev` judged against `lock.rev`), though the terminal may name `--editorial` for it.

**Mocks** (`mocks/ledger-api/fixtures.ts`), all but `empty`: PR-0010, two 15-key rows; PR-0011, two 17-key rows, a `spec_ahead same_revision` (Editorial) and a moved `conflict` with levels.

## Tree badges

"Show drift", off by default, kept per viewer and project in `localStorage` (`specengine.tree.show-drift.<project>`; unreadable: off). On, once the tree read answered (a cached answer counts), the Tree observes `["health", p]`: read on entry and on turning it on, then Health's triggers (`health.md` "UI"). `tree/drift.ts` counts a line's own row 6 rows by `namesOf`, bytes as sent, never a parent's: badges Fix, Check, Cannot verify. `tree/DriftStatus.tsx` says why none show and sums rows naming no line, linking to Health. Texts: `docs/features/ui-tree-drift.md`.

## Known limits

- No deep link to a tab; no trigger of its own: the tab is stale after a code edit until "Read again", re-entry or an apply.
- With Show drift on, each Tree entry, toggle-on and `proposal.approved`, `.rejected`, `.apply_failed`, `.applied` or gap event runs a whole Health walk (rows 3-6, `health-drift.md` "Known limits"), node and search reads waiting in the project's turn; the first three move no badge.
- An ID with two holders counts its rows on both lines, as the inbox counter does.
- `DriftPart` and `codeRows` load with the main chunk, the Inbox card pulling in `DriftPart` too (474.46 kB, under Vite's 500 kB warning).
- A card's Editorial mark and facts are as of `base_commit`, so may be stale against `HEAD`; the terminal judges again.

## Tests

UI: `NodePane.test.tsx` (tab order, the one read, rows, states, commands, one-line words), `commands.test.ts`, `tree.smoke.test.tsx`, `tree/drift.test.ts`, `TreeView.test.tsx` and `TreeView.renders.test.tsx` (badges, line, toggle, renders), `corpus.test.ts`, `live.test.tsx`; `inbox/drift.test.tsx` (the guard, every fact text, raw rows, commands, the mark), `mocks/drift.smoke.test.tsx` (PR-0010, PR-0011), `HealthView.test.tsx` and `mocks/health.smoke.test.tsx` (row 6's details: stub rows, harbor-sim); `policy.test.ts` (AC-U6, no sort), `daemonKeys.test.ts` (`StoredDriftRow`), `http.test.ts`. Rust: http `daemon_keys.rs`, `drift.rs`, `norm.rs`; CLI `kind_amendment.rs` (the count).
