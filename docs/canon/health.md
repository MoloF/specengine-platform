---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-10-19
---

# Health: what is left, measured

The corpus-wide read behind Health's "What is left" (06 s7). Row 3 is `amendments` (the unapplied amendments of `proposal-amendment.md` "Unapplied"), described here; rows 4-5 are `assumes` (`@assumes` against a decision) and `unbound` (unbound selected nodes) beside the `code` count, in `health-code.md`. The document **grows by keys**, one per producer (row 6, drift: 9b), never a placeholder before it. Rows 1-2 are the queue (`spec inbox`, the Inbox's order: severity, `created_at` oldest first, ID; no age threshold, no clock); row 7 is the check's `budget` findings. Informational (rows 4-5 write the data directory's indexes only): it never blocks, raises no check finding, rule or gate (ADR-0012); the core names no class, status, kind or link type (ADR-0008). No MCP tool or resource, no plugin skill.

## Rule

Core `check::unapplied_amendments(&CheckInput) -> Unapplied {amendments: Vec<&CheckFile>, not_judged: Vec<(&CheckFile, &Diagnostic)>}` (`check/unapplied.rs`): every walked file that is live (`is_live`: neither Tier 3 by status nor `class: generated`) and whose `amended_refs` is non-empty, whatever raised it (by hand too), its class or how its links fare. A file whose front-matter does not read (`front_matter_failure`: not UTF-8, unclosed, bad YAML, no mapping) is **not judged**: listed in `not_judged` with that finding, never taken for none. Both lists by path, byte order, blind to input order. A file that could not be read is the walk's gap, which the caller stops on first.

Listed: a spec `draft` or `in-progress`, a decision `accepted` or without `status:`, a canon. Not listed: a spec `shipped` or `abandoned`, a decision no longer `accepted`, a generated file, a document with other link types only, an amendment still a queue proposal (its file is not in the tree).

## spec health

`spec health [--json]`, global flags only (CLI `health(&Env, &Globals) -> HealthOutcome`, `Outcome::Health`). Input as `spec check`'s: `locate`, the store's `load_config`, `WorkingTree`, `check_input` (uncommitted, untracked and ignored files the walk takes). Row 3 reads no database, index, queue, git, clock or baseline. Rows 4-5 (`code`, `assumes`, `unbound`) follow it from the same config bytes, refreshing and reading the code index as `spec refs` does, nothing opened without `[code]`: `health-code.md` (ADR-0051). Links: `SpecGraph::new` over the same input, built only when a document is listed; each document's out links as `spec show <path> --links` lists them (its own file admitted, no Tier 3 one), those of the model's `AMENDS` type: exactly its `amended_refs`, every state (`resolved`, `dangling`, `skipped`, `unchecked`), in `show --links` order. Uncut.

**Text**, each line one-lined: per amendment `<path>[ | id <id>][ | class <class>][ | status <status>][ | <title>]` (absent parts omitted), then each link as `show --links` prints an out line, two spaces in; then rows 4-5's lines (`health-code.md` "Text"); last, always: `spec health: <d> documents, <n> unapplied amendments`.

```text
docs/decisions/DEC-0014.md | id DEC-0014 | class decision | status accepted | Quay lights stay amber
  out amends RULE-QUAY-LIGHTS | docs/decisions/DEC-0014.md:8 | dangling: `RULE-QUAY-LIGHTS` resolves to no ID and no alias
spec health: 41 documents, 1 unapplied amendments
```

**JSON**: `{documents, amendments, notes, code, assumes, unbound}` (the last three: `health-code.md` "Document"); an amendment `{path, id, title, class, status, links}`: `id` the document's Latin ID, `title` `title:` else the first H1, `class` and `status` raw, each `null` when absent; `links` the `ShownLink` of `show --links` (`spec-cli-graph.md` "spec show --links"), never empty. `documents` = the walked files = `spec check --json`'s `counts.documents`. Every key present, absent `null`, empty `[]`.

**Notes**: one per file not judged, by path: `<path>:<line>: <code>: <message>; not judged` (the parser's code and message), on stderr as `note: ` in both modes and in JSON `notes`; exit 0.

**Exit**: 0 answered, whatever is listed, zero too. 2 could not run, stdout empty, never a partial list: usage; discovery (no `specengine.toml`: `spec init`); a config error, `<config>:<line>: message` per cause; `spec: the root cannot be read: <error>: unapplied amendments not measured`; a walk gap (`walk_gap`, the first by path) `spec: the walk is incomplete: <gap>: unapplied amendments not measured`, `<gap>` as `spec export index` words it: `` `<path>` cannot be read (<error>) ``, `` the directory `<path>` cannot be listed ``, `` the `[paths] roots` entry `<path>` names no directory and no `.md` file ``. Never 1.

**Determinism**: one tree and config give byte-identical stdout, stderr and JSON, whatever the absolute root, walk order or starting directory.

## Route

`GET /api/projects/:p/health` (`specengine-http`, `crates/specengine-http/README.md` "Endpoints"): 200 `spec --root R health --json`'s stdout less its final LF. Error bodies `{"status", "message"}`: 400 any query name (it takes none) or a bad `%`, naming it; 403 the fence; 404 an unknown slug or route (the route list names `health` after `check`); 405 any other method, HEAD too, `Allow: GET`; 500 a caught panic; 503 exit 2 (a config `discover` refuses, a changed slug, a walk gap), `message` the CLI's line(s) verbatim. `Cache-Control: no-store`, no `Access-Control-*`. It runs in the project's turn (`serve.md` "Concurrency"), writes only the data directory's indexes (rows 4-5), emits no event; any state of rows 4-5 is a 200.

## UI

Health's "What is left" (`ui/src/health/`): "Proposals in the queue" (rows 1-2), "Unapplied amendments" (row 3, `AmendmentsPart.tsx`), "Code assuming a decision" (row 4, `AssumesPart.tsx`), "Unbound nodes" (row 5, `UnboundPart.tsx`; frame `codeRows.tsx`), "Not measured yet" (row 6, "Phase 3", no digit); row 7 is Budgets. The queue has its own read; rows 3-5 share `getHealth`, each its own boundary: one failing leaves the others. Row 3 keeps three states apart:

- **being measured**: a skeleton, `aria-busy`, no digit;
- **could not be measured** (a 503, an older daemon's 404, status 0): "The unapplied amendments could not be read", the daemon's message verbatim, a Retry reading `getHealth` alone; no `.health-value`, never 0;
- **measured**: "<n> unapplied amendment(s) over <d> document(s)"; rows in the daemon's order: the path linked to the tree, the title, ID, class and status raw; per link its type, `name` linked to the tree else `written` as text, `linkStateLook(state)`'s label and icon, `path:line`, `at`, `as`, the reason verbatim; over 10 rows (`SHOWN_OPEN`) the first 10 and "Show all <n>", which focuses the first added row. Zero: "None: no live document of this tree declares an `amends` link." then "Measured over <d> documents.", never "nothing left". Notes: "Notes from the daemon on the unapplied amendments".
- **rows 4-5** (`health-code.md` "Document"): being measured as row 3; **not measured** (`off`, `not_configured`, an unlisted word): "Not measured: <label>", the causes, no digit, no Retry; a `measured` state with `counts` or `rows` null: "Not measured: the answer holds no counts" (`labels.ts` `MEASURED_WITHOUT_COUNTS`, by the look's `measured`), the causes, no digit, no Retry; **could not be measured** (`cannot`): the causes, a Retry reading `getHealth` alone; **the read failed**: the daemon's message verbatim, shown, not spoken (row 3 speaks it), Retry; no `.health-value`, never 0; **measured**: "<rows> rows of <markers> markers", "<unbound> of <selected> selected nodes"; zero "None: …" then "Measured over <markers> markers in <code.files> code files." / "Measured over <selected> selected nodes.". Lines in the daemon's order, `SHOWN_OPEN` and "Show all <n>" as row 3; fields and causes one-lined, controls `\u{..}` through `ui/src/ui/text.ts` (`shownText`, `shownSlot`, shared with the Inbox and Questions) as `health-code.md` "Text", U+2028/U+2029 too; paths plain. Row 4: the state's badge, the ID as written linked to its inbox card (item found), to the tree at `node.id` (an `@assumes` on a node), else text; `path:line:column`, relation, the CLI's tail. Row 5: the ID to the tree, kind and title raw, `path:line`, "Rules failed", mentions; per marker its place, relation, ID state, binding, reason. Words through `labels.ts` (`codeRowLook`, `assumedLook`, `unboundReasonLook`, `idStateLabel`, `bindingLabel`), an unlisted one raw; tones `--assumed-*` (`--assumed-otherwise` is `--sync-predated`, #f59ac2: nine states, nine colours), `--unbound-*`, `--unbound-cannot-verify` cannot-verify's, never ok's.

`getHealth(project)` (`HealthReport`, `UnappliedAmendment`, the existing `ShownLink`; provisional, citing `docs/features/health-left.md` "Data", rows 4-5's `docs/features/health-code.md` "Data"), key `["health", p]` (`useHealth`), read on entering Health, on "Check again" and, while Health is on screen, after `proposal.approved`, `.rejected` or `.apply_failed` (the health read alone, never by `step`/`reason`), `proposal.applied` or a gap (with the spec reads); never on `proposal.created`, a stage, `task.*`, focus, reconnect or an interval; no `AbortSignal`. A trigger during a walk in flight (the query's pending fetch, fetching or paused) keeps it (`cancelRefetch: false`) and owes one read however many land; when the walk settles, answered or failed, `useLiveQueue` invalidates `["health", p]` once more (exact, active only: none off screen); a failed trailing read draws as any failure (an answer on screen: row 3's "could not be read again" note) and schedules nothing; no timer; the subscription's cleanup drops the debt. The check keeps its rule (kept, answer taken, no trailing read). The home reads none. Mocks: harbor-sim's two (a decision with a dangling link, a draft spec resolved, both in its mock corpus), ledger-api none; `empty`, `large` (3 000), `cannot-check` (a walk-gap 503), `error`, `slow`; rows 4-5: harbor-sim every row-4 state and row-5 reason, ledger-api both `off`; `empty` both zero; `large` 3 000 unbound; `?scenario=` `code-cannot` (rows 4-5 `cannot`, row 3 measured), `not-configured` (row 5), two `ui/README.md`'s list omits.

## Known limits

- A lower bound: a file whose front-matter does not read is only noted; a mistyped `links:` (a scalar `amends:`) reads as none (`proposal-amendment.md` "Known limits"); a file or directory whose name is not UTF-8 is skipped by the walk (`name-skipped` in `spec check`), no gap and no note here.
- A document in a class that never reaches Tier 3 (a canon) declaring `amends` stays listed until edited.
- The served root's working tree only, no other worktree. A project writing amendments in its own vocabulary reads none until its importer maps them (ADR-0008).
- A hand edit fires no event: "Check again" or re-entering Health re-reads.
- Row 4 is not read again on `proposal.created` (`in_question`; `unknown` to `pending`) until "Check again", re-entering Health or another trigger. A run killed after its commit but before apply step 10 fires no further event: "Check again" re-reads. A step-10 `apply_failed` re-reads neither the tree nor the check. The check's walk in flight is kept, its answer possibly older than the event.
- Rows 4-5's: `health-code.md` "Known limits".
- Entering Health runs two whole-tree walks (check, health), Health's index refreshes and the inbox, serialized by the turn: 0.11 to 0.32 s per health walk at 150 to 1 540 documents (release).
- Untested: the 500; the unreadable-root line (the config read fails first); no event through an SSE probe (`door.rs` covers it indirectly).

## Tests

CLI `tests/health.rs` (rule, links parity, keys and text, gaps, notes, read-only and determinism, genre; rows 4-5: `health-code.md` "Tests"); http `tests/health.rs` (parity, refusals, 503s), `turn.rs`, `door.rs`, `daemon_keys.rs` (56 key sets); UI `health.smoke`, `health.live` (triggers, the trailing read, a failed walk), `HealthView`, `mocks/health`, `daemonKeys`, `policy` (`round/text` confined to `src/round/`), `labels`, `tokens` (nine `--assumed-*`), `MockClient`, `ui/text` (U-09) tests.
