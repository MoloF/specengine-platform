---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-model, crates/specengine-store]
owner: owner
reviewed: 2026-10-21
---

# Code in reads: binding rows, their sync, the degrade

A read of nodes carries each of their code bindings as one row whose `sync` is the state `spec verify --node` gives it, by construction; the read never fails on the code layer (`architecture-code.md#sync-in-reads`, ADR-0046). Built: `spec show --bindings`, its twins, `delta.tests_reaching` (7a-1, `docs/features/show-bindings.md`); the bundle's layers 5 and 9, the package's `bindings` (7a-2, `docs/features/bundle-bindings.md`); `spec impact` (7b-1, `spec-impact.md`) reuses row, seam and degrade. Code: CLI `bindings.rs`, `show.rs`, `cap.rs`, `bundle.rs`, `package.rs`, `node_set.rs`; model `task.rs`; store `code.rs`.

## The binding row

`specengine_model::BindingRow` (with `BindingAnchor`, `BindingPlace`; words as strings): 14 keys in this order, all present, absent `null`.

| Key | Value |
|---|---|
| `node`, `relation`, `file`, `kind`, `symbol`, `item_kind` | verify's row key (`spec-verify.md` "Rows"): `node` the canonical cited ID its markers or lock entry resolve to (unresolved: as written; a bare marker `""`); keyless: the last three `null` |
| `line`, `end_line` | the stored symbol's (1-based, attributes excluded) whose `qpath` is `symbol` and `item_kind` is `item_kind`, in the BLAKE3-checked selected file; `ron`: `line` the first marker's, `end_line` `null`; no such symbol (a file module, keyless, gone): `null` |
| `signature`, `layer` | that symbol's (`symbol-names.md` "Signature"; `layer` `"a"` today), else `null` |
| `sync`, `category`, `detail` | verify's state word, category, detail (`spec-verify.md` "States"); `null` when not judged |
| `anchor` | verify's `{state, from, candidates}` (`spec-verify-output.md` "JSON"), `null` where verify's is and when not judged |

Verify's `class`, `pre_existing`, `markers`, `node_rev`, `lock`, `compared`, `levels` are left out (`check_binding` gives them). Order: verify's (core `EntryKey`'s, keyless first), never rowid.

```json
{"node":"LAW-BOUND","relation":"implements","file":"crates/tally/src/counter.rs","kind":"item",
 "symbol":"crates/tally::counter::impl Tally #1","item_kind":"impl_item","line":22,"end_line":38,
 "signature":"impl Tally","layer":"a","sync":"code_ahead","category":"code_changed","detail":"body",
 "anchor":{"state":"exact","from":null,"candidates":null}}
```

## The node sets

- `show --bindings`: every shown node (REF's holders) and every section within it (`node_set.rs` `contained`; a document: all of them); a node lists the rows of {itself} ∪ its sections, a row under each shown node whose set holds it; one judge per read, over the union. A shown node whose ID the corpus read again no longer holds (its file changed between the reads) lists none.
- `verify --node`: REF's holders only.
- A row is a set's as `spec-verify-scope.md` "`--task`" keeps it (an alias's, a look-alike's, a lock-only `marker_gone` row too); a parent's, a sibling's, a mention never.

## The seam

CLI `bindings.rs`, one judge for `verify --node` and the reads:

- `judge_set(&SetScope {project, code, input, lock, keys}, …) -> Result<SetJudged, RunCause>`: `spec-verify-scope.md` "`--node`" steps 4-6 over any node set (levels hashed afresh, never the index's digest columns), `code_selection` also giving each selected file's stored symbols (`SelectedFile.symbols`, by `ord`: the row's lines, signature, layer); rows kept by `verify.rs` `key_set_nodes`. `verify --node` calls it over its holders.
- `set_bindings(project, text, git_env, index, input, keys)`, `set_bindings_with(.., tested_also)` -> `SetBindings {rows, judged, note, code, tested}` (`code` the walk's counts, `tested` `SetTested`s: `spec-lock-tested.md` "Reads"): "Degrade", never a read's failure. `verify_rows.rs` `list_rows` lists keys as the judge gathers them: nothing hashed, git never asked, no pairing.

## Degrade

A read's exit never changes because of `sync` or a degrade cause; `show --bindings`' spec part never because of code (the bundle's rows are items of its fit: "The bundle and the package"). Causes: verify's run causes, numbered as in `spec-verify.md` "Verdict and exit"; `[verify]` is never read.

| Cause | Rows | Note |
|---|---|---|
| (2) no `[code]` | `null`, nothing else read | `note: bindings: <config> has no [code] table: the code layer is off (add [code] with roots)` |
| (1) a `[code]` error, an unknown `frameworks` name; (3) a walk gap (the refresh's, or a file changed twice); (8) the code refresh failing on the data directory or store | `null` | `note: bindings: <cause>` |
| (4) no worktree, no `git`, a git failure; (5) `spec.lock` unmerged; (6) it is unreadable or invalid (format 1, other recipes) | listed, never judged: the selected markers' keys plus the cut lock's entries when its bytes parse (`parse_lock`, any format or recipe); the judgement keys `null`; unpaired, so a moved binding is two rows | `note: bindings: <cause>` |
| none | judged (`cannot_verify` too) | none |

- One note per read, `<cause>` the `cannot:` line without `cannot: `. (2), (1) come from the config, (3), (8) from the code refresh: each ends the bindings read at once, so the note names it even when git or the lock is also faulty (verify names (4)-(6) before (8); the rows are `null` either way). Else the first cause in verify's order; the git session's end failing is (4) only when no cause is listed yet, as in `verify --node`. Any of (1), (2), (3), (8): rows `null`.
- The note follows the read's own messages, on stderr and in the JSON's `notes`; the code refresh's notes are not printed. A row not judged is never `ok`.

## spec show --bindings

`spec show REF --bindings [--links [--archive]] [--json]`. The md index as `show`; with a `[code]` table the spec files read again whole (as `--links`); once REF named a node, the code index refreshed once (`refresh_code`). `show`'s refusals and exit 1 come first, the code layer unread; without the flag code is never touched. Git as `verify --node` (`rev-parse`, `hash-object` without `-w`), never `status` or `diff`; writes the data directory only.

A node's block follows its header and links block, before its bytes:

```text
  LAW-BOUND | implements | crates/tally/src/counter.rs:22-38 | item crates/tally::counter::impl Tally #1 impl_item | layer a | sync code_ahead code_changed body
    sig impl Tally
  bindings 1, 1 check
```

- Row: `  <node> | <relation> | <place> | <binding> | layer <layer> | sync <sync>[ <category>][ <detail>]`; `<place>` `<file>:<line>-<end_line>`, `<file>:<line>` or `<file>`; `<binding>` verify's (keyless `<state> <category>`); `-` for `null`. Under it, four spaces in: `sig <signature>`; `moved from <file> | <binding>`; `candidate <file> | <binding>` per candidate.
- Closing: `  bindings <n>[, <f> fix][, <c> check][, <k> cannot]` (zeros left out); listed rows `  bindings <n>, sync -`.
- JSON: a node's `bindings`, its 19th key, after `links`: `null` without `--bindings`, without `[code]` or under a no-row cause; else `{"rows": […], "omitted": <k>, "tested": […]}`.
- Cap (`spec-cli-read.md` "The cap"): the block counts with the links block; a cut in it falls after the last whole row (with its lines) or the closing line that fits, the text unshown; the tail gains `; bindings not shown: <k>` when any node's `bindings` is not `null`. The daemon's `View::Browser`: uncut.
- Below the cap the spec part (the text without bindings blocks; the JSON without `bindings` and the note) and the exit are `spec show REF`'s. Near it the block takes the text's room, and the JSON follows the text's cut whenever a node's `links` or `bindings` is not `null`: `rows` the printed, `omitted` the tail's `<k>` on the cut node, 0 elsewhere. Without `[code]`: `spec show REF`'s output plus the note.

## The bundle and the package

`spec bundle` and `spec task show`, with their twins, carry rows; layers and fit: `spec-cli-bundle.md`; key, brief, caps: `task-package.md`.

- **Sets**, by node, never by file: layer 5 `bindings`, the non-`verifies` rows of each target and every node within it (`SpecGraph::within`); layer 9 `tests`, the `verifies` rows of those and of every layer-4 candidate, placed or not; the package's `bindings`, every relation, of `targets`, `affected_nodes` and the sections each contains (`node_set.rs` `named_set`, resolved now as `spec show` resolves a REF; one naming nothing adds none; criteria refs not: layer 9 has their tests). A row is a set's as "The node sets" says, in verify's order; tests never run.
- **One read**: with `[code]` one index handle (`open_index`, `indexed_in`) for the spec read and the one code refresh, one `set_bindings` over the union (`bundle.rs` `code_rows`), the package's set in it (`bundle_with(…, Some(&mut Beside {refs, tested_refs, rows}))`). A bundle stopping before its code read (a target naming nothing now) leaves `rows` `None`, and `beside_rows` reads the set alone; one stopping after (the minimum, the ceiling) keeps its rows. Rows are the reading root's, as `bundle_hash`. Without `[code]` code is never read: no rows, heading or note, every output as before.
- **Degrade**: one `bindings: <cause>` note per command; (1), (3), (8): no rows, body and hash the no-`[code]` bundle's; (4)-(6): rows `sync -`, fitted as judged ones.
- **Bundle form**: `spec show --bindings`' row line without its indent (`cap.rs` `row_line`), then `  sig <signature>` when not `null` (`anchor` in the JSON only); an empty line between rows. An item, in or out whole; JSON: the 14 keys, `tokens_est` (its lines'); left out, a tail entry named by its symbol, else its file.
- **Package cap**: at most `PACKAGE_BINDINGS_MAX` 64 (core `task.rs`), the first in row order; past `PACKAGE_BUDGET` rows leave from the last, after the diff and reference texts (`within_budget`); both in one note, last: `bindings: <n> more row(s) left out; spec show <node> --bindings lists them`, `<node>` the first left-out row's.
- **Brief**: Bindings, 10th of 11 sections, before Bundle (bulky rows never push earlier sections past the cut), only with rows or entries: `Bindings: <n>`, each row as `cap.rs` `row_lines` prints it (`sig`, `moved from`, `candidate` too). It renders the JSON alone (`render_brief(&TaskPackage)`): a binding is `row_binding`'s: the key, else `<sync>[ <category>]`, `-` unjudged (body, `show`: verify's text).

## Twins

- MCP `get_node {id, with?, archive?}`: `with` items the enum `["links", "bindings"]`; `archive` with `"links"` only. Mirror: `ShowNode.bindings` `Option<ShowBindings {rows, omitted, tested}>`, `BindingRowShape` key for key (`relation`, `kind`, `sync`, `anchor` verify's enums; the rest free). `NODE_DESCRIPTION` 1 670 B. `get_context_bundle`'s `layers.bindings`, `.tests`: `BundleRowShape` (`BindingRowShape`'s keys, then `tokens_est`); `get_task`'s `bindings`: `BindingRowShape`. The resource `spec://<slug>/node/<REF>` carries none.
- Daemon: `nodes/{*ref}?with=bindings`, repeatable with `with=links`: the CLI's JSON, `View::Browser`; another value: 400 `` `with=<value>`: the values are `links` and `bindings` ``. The bundle's and task's JSON carry rows; the Bundle tab prints them as lines with a Sync badge, no link (`docs/features/bundle-bindings.md` "UI and daemon keys"); no node screen (RM-0011).
- Plugin: `read-spec` names `with: ["bindings"]` (0.13.0), the bundle's rows and their `sync` (0.14.0, step 1; `plugin.md` "Skills").

## delta.tests_reaching

`spec verify --json` lists the tests reaching the affected nodes in every mode but `--node` (`null`: only its files are judged): `spec-verify-output.md` "JSON". `check_binding` keeps `{"type": "null"}` (`mcp-code.md` "Mirrors").

## Known limits

- Tree counters (`spec tree`, `get_tree`, the daemon's tree; ADR-0046 names them) are deferred to RM-0011, opt-in: by default every tree load would judge the whole tree.
- Cost, release, warm, `fixtures/code-a` plus 1 000 files (`show_bindings_cost`, `#[ignore]`): `show --bindings` 51 ms, `verify --node` 52, plain verify 64, plain `show` 6. With 10 000 files (`bundle_bindings_cost`): `bundle` 219 ms (7 without `[code]`), `task show` 237, plain verify 365.
